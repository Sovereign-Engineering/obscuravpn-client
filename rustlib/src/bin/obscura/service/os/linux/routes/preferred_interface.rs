use anyhow::{Context, anyhow, bail};
use futures::{StreamExt, TryStreamExt};
use obscuravpn_client::net::NetworkInterface;
use obscuravpn_client::positive_u31::PositiveU31;
use obscuravpn_client::tokio::AbortOnDrop;
use rtnetlink::RouteMessageBuilder;
use rtnetlink::constants::RTMGRP_IPV4_ROUTE;
use rtnetlink::packet_route::link::{LinkAttribute, LinkMessage};
use rtnetlink::packet_route::route::{RouteAttribute, RouteHeader, RouteMessage};
use rtnetlink::sys::{AsyncSocket, SocketAddr};
use std::convert::Infallible;
use std::net::Ipv4Addr;
use std::time::Duration;
use tokio::select;
use tokio::sync::watch::{Receiver, Sender, channel};
use tokio::time::sleep;

const PREFERRED_INTERFACE_WATCH_ERROR_BACKOFF: Duration = Duration::from_secs(1);

pub async fn watch_preferred_network_interface() -> Receiver<Option<NetworkInterface>> {
    let (sender, receiver) = channel(None);
    tokio::spawn(async move {
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
    let abort_handle = AbortOnDrop::spawn(connection);
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
                RouteAttribute::Oif(v) => interface_index = Some(PositiveU31::try_from(v).context("interface index out of range")?),
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
            .match_index(interface_index.into())
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
