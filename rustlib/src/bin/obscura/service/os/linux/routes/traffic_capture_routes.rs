//! Policy routing that captures all traffic into the tunnel while connected, except the service's own sockets (relay UDP, API HTTP), which carry our fwmark and bypass the capture to reach the physical network. While capture is engaged, an enforcer task installs the capture route (`default dev <tun>`) in our own table and policy rules per address family, and restores them if anything else removes them (NetworkManager can't be trusted):
//! - pref 14999: one rule per tunnel resolver IP, sending it to our capture table before the main table is consulted, so a local route covering the resolver IP can't pull DNS out of the tunnel. Only present while we configure DNS ourselves.
//! - pref 15000: lookup main, but treat a default-route-only match as no match (suppress_prefixlength 0), so every route more specific than a default keeps working.
//! - pref 15001: send everything without our fwmark to our capture table. Marked service traffic skips it and uses the untouched main table default route.
//!
//! The complete routing state, connected, IPv4 (IPv6 via `ip -6` is identical):
//!
//! ```text
//! $ ip rule
//! # Not ours, kernel default.
//! 0:     from all lookup local
//! # Tunnel resolver always goes into our table.
//! 14999: from all to 10.64.0.1 lookup 1868723043 proto 111
//! # Use main table routes more specific than a default route.
//! 15000: from all lookup main suppress_prefixlength 0 proto 111
//! # Capture all unmarked traffic into our table. The table id is the fwmark.
//! 15001: not from all fwmark 0x6f627363 lookup 1868723043 proto 111
//! # Not ours, kernel default. Only marked traffic gets here and uses the untouched default route.
//! 32766: from all lookup main
//! # Not ours, kernel default.
//! 32767: from all lookup default
//!
//! $ ip route show table 0x6f627363
//! # The capture route: everything into the tun device, no destination, no gateway.
//! default dev obscuravpn proto 111
//! ```

use crate::service::os::linux::TrafficPolicy;
use futures::StreamExt;
use obscuravpn_client::net::{FWMARK, NetworkInterface};
use obscuravpn_client::tokio::AbortOnDrop;
use rtnetlink::constants::{RTMGRP_IPV4_ROUTE, RTMGRP_IPV4_RULE, RTMGRP_IPV6_ROUTE};
use rtnetlink::packet_route::AddressFamily;
use rtnetlink::packet_route::route::{RouteAttribute, RouteHeader, RouteMessage, RouteProtocol};
use rtnetlink::packet_route::rule::{RuleAction, RuleAttribute, RuleFlags, RuleMessage};
use rtnetlink::sys::{AsyncSocket, SocketAddr};
use rtnetlink::{IpVersion, RouteMessageBuilder};
use std::collections::BTreeSet;
use std::convert::Infallible;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;
use tokio::select;
use tokio::sync::watch::{Receiver, Sender, channel};
use tokio::time::sleep;

const ROUTE_ENFORCER_ERROR_BACKOFF: Duration = Duration::from_secs(1);
const ROUTE_ENFORCER_APPLY_COOLDOWN: Duration = Duration::from_secs(1);

const ROUTE_TABLE: u32 = FWMARK;
const ROUTE_PROTOCOL: u8 = 0x6f;
const RULE_PREF_RESOLVER: u32 = 14999;
const RULE_PREF_SUPPRESS: u32 = 15000;
const RULE_PREF_CAPTURE: u32 = 15001;

pub async fn spawn_route_enforcer(tun: NetworkInterface) -> Sender<TrafficPolicy> {
    let (sender, mut receiver) = channel(TrafficPolicy::Disengage);
    tokio::spawn(async move {
        let mut desired = receiver.clone();
        loop {
            select! {
                _ = async { let _ = receiver.wait_for(|_| false).await; } => {
                    tracing::warn!(message_id = "kD6mWv2H", "route enforcer sender dropped/closed");
                    return;
                }
                Err(()) = enforce_routing(&tun, &mut desired) => sleep(ROUTE_ENFORCER_ERROR_BACKOFF).await,
            }
        }
    });
    sender
}

async fn enforce_routing(tun: &NetworkInterface, desired: &mut Receiver<TrafficPolicy>) -> Result<Infallible, ()> {
    const RTNLGRP_IPV6_RULE: u32 = 19;
    const RTMGRP_IPV6_RULE: u32 = 1 << (RTNLGRP_IPV6_RULE - 1);
    const GROUPS: u32 = RTMGRP_IPV4_ROUTE | RTMGRP_IPV6_ROUTE | RTMGRP_IPV4_RULE | RTMGRP_IPV6_RULE;

    let (mut connection, handle, mut messages) = rtnetlink::new_connection().map_err(|error| {
        tracing::error!(message_id = "wQ2nRv7X", ?error, "failed to create netlink connection");
    })?;
    connection.socket_mut().socket_mut().bind(&SocketAddr::new(0, GROUPS)).map_err(|error| {
        tracing::error!(message_id = "eJ6tJm3V", ?error, "netlink socket bind failed");
    })?;
    connection.forward_unsolicited_messages();
    let _connection = AbortOnDrop::spawn(connection);
    loop {
        while let Ok(Some(_)) = messages.try_next() {}
        let policy = desired.borrow_and_update().clone();
        if routing_dirty(&handle, tun, &policy).await? {
            apply_routing(&handle, tun, &policy).await?;
            sleep(ROUTE_ENFORCER_APPLY_COOLDOWN).await;
            continue;
        }
        select! {
            Ok(()) = desired.changed() => {}
            message = messages.next() => {
                if message.is_none() {
                    tracing::error!(message_id = "pL4wSc9D", "netlink event stream closed");
                    return Err(());
                }
            }
        }
    }
}

const FAMILIES: [(IpVersion, AddressFamily); 2] = [(IpVersion::V4, AddressFamily::Inet), (IpVersion::V6, AddressFamily::Inet6)];

fn wanted_resolver_rules(policy: &TrafficPolicy, family: AddressFamily) -> BTreeSet<IpAddr> {
    match policy {
        TrafficPolicy::Engage { dns, local_network_access: _ } => dns.iter().copied().filter(|ip| address_family(*ip) == family).collect(),
        TrafficPolicy::Disengage => BTreeSet::new(),
    }
}

async fn routing_dirty(handle: &rtnetlink::Handle, tun: &NetworkInterface, policy: &TrafficPolicy) -> Result<bool, ()> {
    let engaged = matches!(policy, TrafficPolicy::Engage { .. });
    let mut dirty = false;
    for (ip_version, family) in FAMILIES {
        let (mut have_suppress_rule, mut have_capture_rule, mut have_capture_route) = (false, false, false);
        let mut have_resolver_rules = BTreeSet::new();
        let mut rule_dump = handle.rule().get(ip_version.clone()).execute();
        while let Some(rule) = rule_dump.next().await {
            let rule = rule.map_err(|error| {
                tracing::error!(message_id = "gF2xWn9K", ?error, "failed to list rules");
            })?;
            have_suppress_rule |= is_suppress_rule(&rule);
            have_capture_rule |= is_capture_rule(&rule);
            have_resolver_rules.extend(resolver_rule_destination(&rule));
        }
        let want_resolver_rules = wanted_resolver_rules(policy, family);
        let route_dump_message = match ip_version {
            IpVersion::V4 => RouteMessageBuilder::<Ipv4Addr>::new().build(),
            IpVersion::V6 => RouteMessageBuilder::<Ipv6Addr>::new().build(),
        };
        let mut route_dump = handle.route().get(route_dump_message).execute();
        while let Some(route) = route_dump.next().await {
            let route = route.map_err(|error| {
                tracing::error!(message_id = "jK5nZw3Q", ?error, "failed to list routes");
            })?;
            have_capture_route |= is_capture_route(&route, tun);
        }
        if [have_suppress_rule, have_capture_rule, have_capture_route] != [engaged; 3] || have_resolver_rules != want_resolver_rules {
            tracing::info!(
                message_id = "qX5mBd7R",
                ?family,
                engaged,
                have_suppress_rule,
                have_capture_rule,
                have_capture_route,
                ?have_resolver_rules,
                ?want_resolver_rules,
                "routing state dirty"
            );
            dirty = true;
        }
    }
    Ok(dirty)
}

async fn apply_routing(handle: &rtnetlink::Handle, tun: &NetworkInterface, policy: &TrafficPolicy) -> Result<(), ()> {
    let engaged = matches!(policy, TrafficPolicy::Engage { .. });
    for (ip_version, family) in FAMILIES {
        loop {
            match handle.rule().del(any_resolver_rule(family)).execute().await {
                Ok(()) => {}
                Err(rtnetlink::Error::NetlinkError(message)) if message.raw_code() == -libc::ENOENT => break,
                Err(error) => {
                    tracing::error!(message_id = "mK2xVb9R", ?error, "failed to delete resolver rule");
                    return Err(());
                }
            }
        }
        if engaged {
            let resolver_rules = wanted_resolver_rules(policy, family).into_iter().map(resolver_rule);
            for rule in resolver_rules.chain([suppress_rule(family), capture_rule(family)]) {
                let mut request = handle.rule().add();
                *request.message_mut() = rule;
                match request.execute().await {
                    Ok(()) => {}
                    Err(rtnetlink::Error::NetlinkError(message)) if message.raw_code() == -libc::EEXIST => {}
                    Err(error) => {
                        tracing::error!(message_id = "cV4tYm1R", ?error, "failed to add rule");
                        return Err(());
                    }
                }
            }
            if let Err(error) = handle.route().add(capture_route(tun, ip_version)).replace().execute().await {
                tracing::error!(message_id = "yN8wEa5K", ?error, "failed to add route");
                return Err(());
            }
        } else {
            for rule in [suppress_rule(family), capture_rule(family)] {
                match handle.rule().del(rule).execute().await {
                    Ok(()) => {}
                    Err(rtnetlink::Error::NetlinkError(message)) if message.raw_code() == -libc::ENOENT => {}
                    Err(error) => {
                        tracing::error!(message_id = "cN7wYb4S", ?error, "failed to delete rule");
                        return Err(());
                    }
                }
            }
            match handle.route().del(capture_route(tun, ip_version)).execute().await {
                Ok(()) => {}
                Err(rtnetlink::Error::NetlinkError(message)) if message.raw_code() == -libc::ESRCH => {}
                Err(error) => {
                    tracing::error!(message_id = "sD5cQn9M", ?error, "failed to delete route");
                    return Err(());
                }
            }
        }
    }
    Ok(())
}

fn address_family(ip: IpAddr) -> AddressFamily {
    match ip {
        IpAddr::V4(_) => AddressFamily::Inet,
        IpAddr::V6(_) => AddressFamily::Inet6,
    }
}

fn resolver_rule(ip: IpAddr) -> RuleMessage {
    let mut rule = RuleMessage::default();
    rule.header.family = address_family(ip);
    rule.header.dst_len = match ip {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };
    rule.header.action = RuleAction::ToTable;
    rule.attributes.extend([
        RuleAttribute::Destination(ip),
        RuleAttribute::Priority(RULE_PREF_RESOLVER),
        RuleAttribute::Table(ROUTE_TABLE),
        RuleAttribute::Protocol(RouteProtocol::Other(ROUTE_PROTOCOL)),
    ]);
    rule
}

fn any_resolver_rule(family: AddressFamily) -> RuleMessage {
    let mut rule = RuleMessage::default();
    rule.header.family = family;
    rule.header.action = RuleAction::ToTable;
    rule.attributes.extend([
        RuleAttribute::Priority(RULE_PREF_RESOLVER),
        RuleAttribute::Table(ROUTE_TABLE),
        RuleAttribute::Protocol(RouteProtocol::Other(ROUTE_PROTOCOL)),
    ]);
    rule
}

fn resolver_rule_destination(rule: &RuleMessage) -> Option<IpAddr> {
    let ip = rule.attributes.iter().find_map(|attribute| match attribute {
        RuleAttribute::Destination(ip) => Some(*ip),
        _ => None,
    })?;
    let expected = resolver_rule(ip);
    let matches = rule.header.family == expected.header.family
        && rule.header.action == RuleAction::ToTable
        && !rule.header.flags.contains(RuleFlags::Invert)
        && rule.header.src_len == 0
        && rule.header.dst_len == expected.header.dst_len
        && rule.attributes.contains(&RuleAttribute::Table(ROUTE_TABLE))
        && rule.attributes.contains(&RuleAttribute::Priority(RULE_PREF_RESOLVER))
        && rule.attributes.contains(&RuleAttribute::Protocol(RouteProtocol::Other(ROUTE_PROTOCOL)))
        && !rule
            .attributes
            .iter()
            .any(|attribute| matches!(attribute, RuleAttribute::FwMark(_) | RuleAttribute::SuppressPrefixLen(_)));
    matches.then_some(ip)
}

fn suppress_rule(family: AddressFamily) -> RuleMessage {
    let mut rule = RuleMessage::default();
    rule.header.family = family;
    rule.header.action = RuleAction::ToTable;
    rule.header.table = RouteHeader::RT_TABLE_MAIN;
    rule.attributes.extend([
        RuleAttribute::Priority(RULE_PREF_SUPPRESS),
        RuleAttribute::SuppressPrefixLen(0),
        RuleAttribute::Protocol(RouteProtocol::Other(ROUTE_PROTOCOL)),
    ]);
    rule
}

fn is_suppress_rule(rule: &RuleMessage) -> bool {
    // Not using `==` because dumps are not echoes of our requests (kernel adds attributes and order is unspecified). Check the fields we know, reject the capture rule's fwmark, but tolerate unknown extras. Even if something else adds lookalikes only routing may break, which does not leak traffic if a kill switch is engaged.
    rule.header.action == RuleAction::ToTable
        && !rule.header.flags.contains(RuleFlags::Invert)
        && rule.header.src_len == 0
        && rule.header.dst_len == 0
        && rule.attributes.contains(&RuleAttribute::Table(RouteHeader::RT_TABLE_MAIN.into()))
        && rule.attributes.contains(&RuleAttribute::Priority(RULE_PREF_SUPPRESS))
        && rule.attributes.contains(&RuleAttribute::SuppressPrefixLen(0))
        && rule.attributes.contains(&RuleAttribute::Protocol(RouteProtocol::Other(ROUTE_PROTOCOL)))
        && !rule.attributes.iter().any(|attribute| matches!(attribute, RuleAttribute::FwMark(_)))
}

fn capture_rule(family: AddressFamily) -> RuleMessage {
    let mut rule = RuleMessage::default();
    rule.header.family = family;
    rule.header.action = RuleAction::ToTable;
    rule.header.flags |= RuleFlags::Invert;
    rule.attributes.extend([
        RuleAttribute::Priority(RULE_PREF_CAPTURE),
        RuleAttribute::Table(ROUTE_TABLE),
        RuleAttribute::FwMark(FWMARK),
        RuleAttribute::Protocol(RouteProtocol::Other(ROUTE_PROTOCOL)),
    ]);
    rule
}

fn is_capture_rule(rule: &RuleMessage) -> bool {
    // Same matching logic as is_suppress_rule, except the roles of suppress_prefixlength and fwmark are inverted.
    rule.header.action == RuleAction::ToTable
        && rule.header.flags.contains(RuleFlags::Invert)
        && rule.header.src_len == 0
        && rule.header.dst_len == 0
        && rule.attributes.contains(&RuleAttribute::Table(ROUTE_TABLE))
        && rule.attributes.contains(&RuleAttribute::Priority(RULE_PREF_CAPTURE))
        && rule.attributes.contains(&RuleAttribute::FwMark(FWMARK))
        && rule.attributes.contains(&RuleAttribute::Protocol(RouteProtocol::Other(ROUTE_PROTOCOL)))
        && !rule
            .attributes
            .iter()
            .any(|attribute| matches!(attribute, RuleAttribute::SuppressPrefixLen(_)))
}

fn capture_route(tun: &NetworkInterface, ip_version: IpVersion) -> RouteMessage {
    match ip_version {
        IpVersion::V4 => RouteMessageBuilder::<Ipv4Addr>::new()
            .table_id(ROUTE_TABLE)
            .protocol(RouteProtocol::Other(ROUTE_PROTOCOL))
            .output_interface(tun.index.into())
            .build(),
        IpVersion::V6 => RouteMessageBuilder::<Ipv6Addr>::new()
            .table_id(ROUTE_TABLE)
            .protocol(RouteProtocol::Other(ROUTE_PROTOCOL))
            .output_interface(tun.index.into())
            .build(),
    }
}

fn is_capture_route(route: &RouteMessage, tun: &NetworkInterface) -> bool {
    // See is_suppress_rule for matching logic.
    route.header.protocol == RouteProtocol::Other(ROUTE_PROTOCOL)
        && route.header.destination_prefix_length == 0
        && route.attributes.contains(&RouteAttribute::Table(ROUTE_TABLE))
        && route.attributes.contains(&RouteAttribute::Oif(tun.index.into()))
}

/// Make the reverse path lookup respect the packet fwmark, so rp_filter (IPv4 only) doesn't throw away inbound relay and API traffic.
/// Ok to set once and never revert: ingress traffic is never marked unless something explicitly marks it, and unmarked packets behave exactly as before, so nothing else is affected.
pub fn enable_src_valid_mark() -> Result<(), ()> {
    std::fs::write("/proc/sys/net/ipv4/conf/all/src_valid_mark", "1").map_err(|error| {
        tracing::error!(message_id = "kR2vHb6T", ?error, "failed to enable src_valid_mark sysctl");
    })
}
