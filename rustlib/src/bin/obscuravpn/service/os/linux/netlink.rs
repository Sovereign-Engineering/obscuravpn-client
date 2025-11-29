use anyhow::{Context, anyhow, bail};
use futures::{StreamExt, TryStreamExt};
use obscuravpn_client::net::NetworkInterface;
use obscuravpn_client::tokio::AbortOnDrop;
use rtnetlink::RouteMessageBuilder;
use rtnetlink::constants::RTMGRP_IPV4_ROUTE;
use rtnetlink::packet_route::link::{LinkAttribute, LinkMessage};
use rtnetlink::packet_route::route::{RouteAttribute, RouteHeader, RouteMessage};
use rtnetlink::sys::{AsyncSocket, SocketAddr};
use std::convert::Infallible;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::num::{NonZeroI32, NonZeroU32};
use std::time::Duration;
use tokio;
use tokio::select;
use tokio::sync::watch::{Receiver, Sender, channel};
use tokio::time::sleep;

const PREFERRED_INTERFACE_WATCH_ERROR_BACKOFF: Duration = Duration::from_secs(1);

pub async fn netlink_connect() -> Result<rtnetlink::Handle, ()> {
    let (connection, handle) = match rtnetlink::new_connection() {
        Ok((connection, handle, _)) => (connection, handle),
        Err(error) => {
            tracing::error!(message_id = "kuk3xhql", ?error, "failed to create netlink connection");
            return Err(());
        }
    };
    tokio::spawn(connection);
    Ok(handle)
}

pub async fn add_routes(tun_idx: u32) -> Result<(), ()> {
    let handle = netlink_connect().await?;
    let route_messages = build_route_messages(tun_idx)?;
    for route_message in route_messages {
        handle.route().add(route_message.clone()).replace().execute().await.map_err(|error| {
            tracing::error!(message_id = "a1Uk0dKX", ?error, ?route_message, "failed to add route");
        })?
    }
    Ok(())
}

pub async fn del_routes(tun_idx: u32) -> Result<(), ()> {
    let handle = netlink_connect().await?;
    let route_messages = build_route_messages(tun_idx)?;
    for route_message in route_messages {
        handle
            .route()
            .del(route_message.clone())
            .execute()
            .await
            .or_else(|error| {
                if let rtnetlink::Error::NetlinkError(error) = &error
                    && error.code == NonZeroI32::new(-3)
                {
                    tracing::info!(message_id = "WGD6Z6xL", "tried to delete non-existent route");
                    return Ok(());
                }
                Err(error)
            })
            .map_err(|error| {
                tracing::error!(message_id = "aDETR9ur", ?error, ?route_message, "failed to delete route");
            })?
    }
    Ok(())
}

/// Build IPv4 and Ipv6 route messages for two routes to the tun device each. The individual routes cover half of the respective address space, which gives them priority over the default route without replacing it. We don't want to replace the default route, because:
/// - We use the default route for preferred network interface discovery
/// - We wouldn't know what the set it to when the tunnel is disabled
/// - Network management services like network manager tend to overwrite it.
pub fn build_route_messages(tun_idx: u32) -> Result<Vec<RouteMessage>, ()> {
    [
        IpAddr::V4(Ipv4Addr::new(000, 0, 0, 0)),
        IpAddr::V4(Ipv4Addr::new(128, 0, 0, 0)),
        IpAddr::V6(Ipv6Addr::new(0x0000, 0, 0, 0, 0, 0, 0, 0)),
        IpAddr::V6(Ipv6Addr::new(0x8000, 0, 0, 0, 0, 0, 0, 0)),
    ]
    .map(|destination| {
        Ok(RouteMessageBuilder::<IpAddr>::new()
            .destination_prefix(destination, 1)
            .map_err(|error| tracing::error!(message_id = "wtAo1gKj", ?error, "failed to build route message"))?
            .output_interface(tun_idx)
            .build())
    })
    .into_iter()
    .collect()
}

pub fn watch_preferred_network_interface(runtime: &tokio::runtime::Handle) -> Receiver<Option<NetworkInterface>> {
    let (sender, receiver) = channel(None);
    runtime.spawn(async move {
        loop {
            select! {
                _ = sender.closed() => {
                    tracing::warn!(message_id = "Vj07sMT5", "preferred network interface receiver dropped/closed");
                    return
                }
                Err(error) = watch_preferred_network_interface_one(&sender) => {
                    tracing::warn!(message_id = "evDsKFvw", ?error, "preferred network interface watcher encountered error");
                    sleep(PREFERRED_INTERFACE_WATCH_ERROR_BACKOFF).await;
                }
            }
        }
    });
    receiver
}

async fn watch_preferred_network_interface_one(sender: &Sender<Option<NetworkInterface>>) -> anyhow::Result<Infallible> {
    let (mut connection, handle, mut messages) = rtnetlink::new_connection().context("failed to create netlink connection")?;
    connection
        .socket_mut()
        .socket_mut()
        .bind(&SocketAddr::new(0, RTMGRP_IPV4_ROUTE))
        .context("netlink socket bind failed")?;
    connection.forward_unsolicited_messages();
    let abort_handle = AbortOnDrop::from(tokio::spawn(connection).abort_handle());
    loop {
        tracing::info!(message_id = "lryydLmq", "routing table changed");
        let new = get_preferred_network_interface(&handle).await?;
        sender.send_if_modified(|current| {
            if *current != new {
                tracing::info!(message_id = "EWxPiQFh", ?current, ?new, "preferred network interface changed");
                *current = new;
                true
            } else {
                false
            }
        });
        if messages.next().await.is_none() {
            break;
        }
    }
    drop(abort_handle);
    Err(anyhow!("netlink route event stream closed"))
}

async fn get_preferred_network_interface(handle: &rtnetlink::Handle) -> anyhow::Result<Option<NetworkInterface>> {
    let mut highest_priority_default_route: Option<DefaultRoute> = None;
    let mut routes = handle.route().get(RouteMessageBuilder::<Ipv4Addr>::new().build()).execute();
    while let Some(route) = routes.next().await {
        let route: RouteMessage = route?;
        if route.header.destination_prefix_length != 0 || route.header.table != RouteHeader::RT_TABLE_MAIN {
            continue;
        }
        let (mut interface_index, mut metric) = (None, None);
        for attr in route.attributes {
            match attr {
                RouteAttribute::Oif(v) => interface_index = Some(NonZeroU32::new(v).context("interface index can't be 0")?),
                RouteAttribute::Priority(v) => metric = Some(v),
                _ => {}
            }
        }
        let (Some(interface_index), Some(metric)) = (interface_index, metric) else {
            bail!("default route with missing oif={interface_index:?} or priority={metric:?}")
        };
        let link_message: LinkMessage = handle
            .link()
            .get()
            .match_index(interface_index.get())
            .execute()
            .try_next()
            .await?
            .context("no matches for interface index")?;
        let interface_name = link_message
            .attributes
            .into_iter()
            .filter_map(|attr| if let LinkAttribute::IfName(name) = attr { Some(name) } else { None })
            .next()
            .context("no name attribute for interface")?;
        let default_route = DefaultRoute { network_interface: NetworkInterface { index: interface_index, name: interface_name }, metric };
        tracing::info!(message_id = "IBDybsCC", "found default route: {default_route:?}");
        if highest_priority_default_route.as_ref().is_none_or(|current| metric < current.metric) {
            highest_priority_default_route = Some(default_route)
        }
    }
    Ok(highest_priority_default_route.map(|r| r.network_interface))
}

#[derive(Debug)]
struct DefaultRoute {
    network_interface: NetworkInterface,
    metric: u32,
}
