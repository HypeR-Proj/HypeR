// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_component::{Reply, Request};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let operation = args.next().unwrap_or_else(|| "list".into());
    if matches!(operation.as_str(), "--help" | "-h") {
        println!(
            "Usage: componentctl [list | status [PATH] | routes PATH [CAPABILITY] | start PATH | stop PATH | restart PATH]\nPaths identify instances (for example /journal). Routes explain each realm boundary.\nStateful bootstrap components require their ownership protocol and cannot be restarted generically."
        );
        return Ok(());
    }
    let path = args.next();
    let request = match operation.as_str() {
        "list" | "status" => Request::Status { path },
        "routes" => Request::Routes {
            path: path.ok_or("routes requires PATH")?,
            capability: args.next(),
        },
        "start" => Request::Start {
            path: path.ok_or("start requires PATH")?,
        },
        "stop" => Request::Stop {
            path: path.ok_or("stop requires PATH")?,
        },
        "restart" => Request::Restart {
            path: path.ok_or("restart requires PATH")?,
        },
        _ => return Err("unknown operation; use componentctl --help".into()),
    };
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    let startup = hyper_rt::process::startup()?;
    let read_only = matches!(request, Request::Status { .. } | Request::Routes { .. });
    let connector = startup.borrow(if read_only {
        hyper_component::OBSERVE
    } else {
        hyper_component::CONTROL
    })?;
    let reply: Reply = hyper_service_ipc::request(connector, &request)?;
    if let Some(error) = reply.error {
        return Err(error.into());
    }
    if operation == "routes" {
        for route in reply.routes {
            println!(
                "{} uses {} from {}:{} ({:?})",
                reply
                    .snapshot
                    .components
                    .get(route.consumer)
                    .ok_or("invalid route consumer")?
                    .path,
                route.local_name,
                reply
                    .snapshot
                    .components
                    .get(route.provider)
                    .ok_or("invalid route provider")?
                    .path,
                route.capability,
                route.kind
            );
            for step in route.steps {
                println!("  {step:?}");
            }
        }
        return Ok(());
    }
    println!("COMPONENT                        STATE         GENERATION RESTARTS PROCESS");
    for component in reply.snapshot.components {
        println!(
            "{:<32} {:<13} {:>10} {:>8} {}",
            component.path,
            format!("{:?}", component.state).to_lowercase(),
            component.generation,
            component.restarts,
            component.process.map_or("-".into(), |p| format!("0x{p:x}"))
        );
        if operation != "list" {
            println!("  {}", component.detail.escape_debug());
            if !component.published.is_empty() {
                println!("  published: {}", component.published.join(", "));
            }
        }
    }
    if !reply.snapshot.progress.is_empty() {
        println!("Startup progress: {}", reply.snapshot.progress.join(", "));
    }
    Ok(())
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("componentctl: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
