// SPDX-License-Identifier: MIT
// Written by: Nathan LeSueur @ Akamai Technologies, Inc.
// Purpose: Updates local routing table with results from DNS lookup
// Intended to be used with LKE-E NAT Gateway 
// Uses proto Babel (numerical 42) to identify routes added by this tool
//
//
use std::{env, net::Ipv4Addr, net::IpAddr};
use hickory_resolver::{Resolver};

use futures_util::TryStreamExt;
use ipnetwork::Ipv4Network;
use rtnetlink::{new_connection, Error, Handle, RouteMessageBuilder};
use netlink_packet_route::route::{RouteAddress, RouteAttribute, RouteProtocol, RouteMessage};

const DNS_ROUTE: RouteProtocol = RouteProtocol::Other(42); 

#[tokio::main]
async fn main() -> Result<(), ()> {
    //Setup connection to rtnetlink
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);
    //Collect command line provided arguments:
    //dnsname record to resolve
    //inteface to use for the route
    //source to use for the route
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        println!("Argument count: {}", args.len());
        usage();
        return Ok(());
    }

    let dnsname: String = args[1].parse().unwrap_or_else(|_| {
        eprintln!("invalid DNS name");
        std::process::exit(1);
    });

    let iface: String = args[2].parse().unwrap_or_else(|_| {
        eprintln!("invalid interface");
        std::process::exit(1);
    });
    //convert the inteface provided on the commandline from string to format to be used with
    //rtnetlink
    let iface_idx = handle
        .link()
        .get()
        .match_name(iface.into())
        .execute()
        .try_next()
        .await
        .unwrap()
        .unwrap()
        .header
        .index;

    let source: Ipv4Addr = args[3].parse().unwrap_or_else(|_| {
        eprintln!("invalid source");
        std::process::exit(1);
    });

    let resolver = Resolver::builder_tokio().unwrap().build().unwrap();
    let response = resolver.lookup_ip(dnsname).await.unwrap();
    let mut run_once = true;
    let current_routes = RouteMessageBuilder::<Ipv4Addr>::new()
        .build();

    //Check IPv4 addresses returned from DNS against the routing table. Verifies IPv4 and
    //gets/creates unified IPv4 address type from DNS resolver and rtnetlink
    //
    //Test each address in the DNS response
    for address in response.iter() {
        //Only concerned about IPv4 (Plan to add IPv6 support if needed later)
        if address.is_ipv4() {
            let mut ipv4address: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);
            let mut routes = handle.route().get(current_routes.clone()).execute();
            if let IpAddr::V4(v4_addr) = address {
                ipv4address = v4_addr;
            }
            println!("Checking DNS results against current table.");
            let ip = RouteAttribute::Destination(RouteAddress::Inet(ipv4address));
            let mut route_found: bool = false;
            while let Ok(Some(route)) = routes.try_next().await {
                let mut dns_proto_exists = false;
                let protocol = route.header.protocol; 
                //We ONLY want to perform oprations on routes with the Babel protocol as we know it
                //is routes from this application.
                if protocol == RouteProtocol::Babel {
                    dns_proto_exists = true;
                }
                for r in &route.attributes.clone() {
                    let mut raddress: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);
                    //There are other attributes in the route record, but we only care about the IP
                    //address for Destination for our comparisons.
                    if let RouteAttribute::Destination(RouteAddress::Inet(x)) = r {
                        raddress = *x;
                    }
                    //If the IP in the DNS result matches a route currently in the table
                    //in_dns_results is "true".
                    let in_dns_results = response.iter().any(|item| match item {
                        IpAddr::V6(_) => false,
                        IpAddr::V4(item) => item == raddress,
                    });
                    if let RouteAttribute::Destination(RouteAddress::Inet(_)) = r && *r == ip && dns_proto_exists {
                        println!("Route exists for {:#?}", ipv4address);
                        route_found = true;
                    }
                    //If the route table has an IP no longer returned in DNS, we need to delete it
                    //as it is considered no longer in use.
                    if let RouteAttribute::Destination(RouteAddress::Inet(_)) = r && *r != ip && dns_proto_exists && run_once && !in_dns_results {
                        let _ = del_route(raddress, route.clone(), handle.clone()).await;
                    }
                }
            }
            //IP reterned from DNS was not found in the routing table, so we need to add it.
            if !route_found {
                println!("No matching result exists. Adding DNS result {} to the table.", ipv4address);
                //Unlikely to be wrong format as the IP source was DNS, but we check it anyway.
                let dest: Ipv4Network = format!("{}/32", address).parse().unwrap_or_else(|_| {
                    eprintln!("invalid destination");
                    std::process::exit(1);
                });
                if let Err(e) = add_route(&dest, iface_idx, source, handle.clone(), address).await {
                    eprintln!("{e}");
                };
            }
        run_once = false;
        }
    }
    Ok(())
}
//Unlike add_route, we have the route payload already from rtnetlink, so we just need to trigger
//the delete
async fn del_route(raddress: Ipv4Addr, route: RouteMessage, handle: Handle) {
    println!("Route exists for {:#?}, but no longer in DNS.", raddress);
    if let Err(e) = handle.route().del(route).execute().await {
        eprintln!("{e}");
    }
    else {
        println!("Route for {:#?} deleted.", raddress);
    }
}
//Build the route payload and add it to the table
async fn add_route(
    dest: &Ipv4Network,
    iface_idx: u32,
    source: Ipv4Addr,
    handle: Handle,
    address: IpAddr,
) -> Result<(), Error> {

    let route = RouteMessageBuilder::<Ipv4Addr>::new()
        .destination_prefix(dest.ip(), dest.prefix())
        .output_interface(iface_idx)
        .protocol(DNS_ROUTE)
        .pref_source(source)
        .build();
    handle.route().add(route).execute().await?;
    println!("Route for {} added.", address);
    Ok(())
}

fn usage() {
    eprintln!(
        "usage:
        dns-to-route <DNS record to resolve> <interface> <source>"
    );
}
