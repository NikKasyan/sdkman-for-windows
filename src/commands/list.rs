use anyhow::Result;
use std::{collections::BTreeSet, path::Path};

use crate::{
    api::{Api, Version},
    cli::Order,
    state::State,
};

pub(super) fn list(state: &State, candidate: Option<String>, order: Option<Order>) -> Result<()> {
    state.init()?;
    let order = order.unwrap_or(Order::Desc);
    if let Some(ref candidate) = candidate {
        super::ensure_candidate_exists(state, candidate)?;
    }
    match candidate {
        None => {
            let api = Api::new(state)?;
            let names: Vec<String> = api
                .candidates(state.config.offline_mode)?
                .into_iter()
                .map(|c| c.name)
                .collect();
            println!("Available Candidates");
            println!("{}", names.join(", "));
        }
        Some(candidate) => {
            let installed = state.installed_versions(&candidate)?;
            let current = state.active_home(&candidate, None)?;
            if state.config.offline_mode {
                println!("Offline Mode: only showing installed {candidate} versions");
                for version in installed {
                    print_list_version(state, &candidate, &version, true, current.as_deref())?;
                }
                return Ok(());
            }
            let api = Api::new(state)?;
            println!("Available {candidate} Versions");
            let mut remote_versions = api.versions(&candidate, false)?;
            super::sort_versions_by_vendor_and_version(&mut remote_versions, order);
            let java_table = candidate.eq_ignore_ascii_case("java")
                && remote_versions.iter().any(|v| v.vendor.is_some());
            if java_table {
                print_java_table_header();
            }
            let mut printed = BTreeSet::new();
            for version in remote_versions {
                let is_installed = installed.contains(&version.value);
                print_list_version_or_java_row(
                    state,
                    &candidate,
                    &version,
                    is_installed,
                    current.as_deref(),
                    java_table,
                )?;
                printed.insert(version.value);
            }
            let mut local_only = installed
                .into_iter()
                .map(Version::local)
                .collect::<Vec<_>>();
            super::sort_versions_by_vendor_and_version(&mut local_only, order);
            for version in local_only {
                if printed.insert(version.value.clone()) {
                    print_list_version_or_java_row(
                        state,
                        &candidate,
                        &version,
                        true,
                        current.as_deref(),
                        java_table,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn print_java_table_header() {
    println!(
        " {:<14} | {:<3} | {:<12} | {:<7} | {:<9} | Identifier",
        "Vendor", "Use", "Version", "Dist", "Status"
    );
    println!("{}", "-".repeat(78));
}

fn print_list_version_or_java_row(
    state: &State,
    candidate: &str,
    version: &Version,
    installed: bool,
    current: Option<&Path>,
    java_table: bool,
) -> Result<()> {
    if java_table {
        print_java_table_row(state, candidate, version, installed, current)
    } else {
        print_list_version(state, candidate, &version.value, installed, current)
    }
}

fn print_java_table_row(
    state: &State,
    candidate: &str,
    version: &Version,
    installed: bool,
    current: Option<&Path>,
) -> Result<()> {
    let use_marker = if installed
        && super::installed_version_is_current(state, candidate, &version.value, current)?
    {
        ">"
    } else {
        ""
    };
    let status = if installed { "installed" } else { "" };
    let vendor = version.vendor.as_deref().unwrap_or("");
    let display_version = version.display_version.as_deref().unwrap_or(&version.value);
    let distribution = version.distribution.as_deref().unwrap_or("local");
    println!(
        " {:<14} | {:<3} | {:<12} | {:<7} | {:<9} | {}",
        vendor, use_marker, display_version, distribution, status, version.value
    );
    Ok(())
}

fn print_list_version(
    state: &State,
    candidate: &str,
    version: &str,
    installed: bool,
    current: Option<&Path>,
) -> Result<()> {
    let installed_marker = if installed { "*" } else { " " };
    let current_marker =
        if installed && super::installed_version_is_current(state, candidate, version, current)? {
            ">"
        } else {
            " "
        };
    println!("{current_marker} {installed_marker} {version}");
    Ok(())
}
