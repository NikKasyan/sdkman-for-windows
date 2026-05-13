use anyhow::{bail, Context, Result};
use clap::CommandFactory;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::{Attribute, Print, SetAttribute},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use reqwest::blocking::Client;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use tempfile::TempDir;

use crate::{
    api::{Api, Version},
    archive,
    cli::{Args, Command, ConfigAction, EnvAction, FlushTarget, OfflineAction, Order},
    config::Config,
    envfile, fslink, shims,
    state::{display_path, session_home_var, InstallRecord, State},
};

#[derive(Serialize)]
struct EnvUpdate {
    set: BTreeMap<String, String>,
    prepend_path: Vec<String>,
    message: String,
}

const ENV_JSON_PREFIX: &str = "__SDKMAN_ENV_JSON__";

pub fn execute(args: Args, state: State) -> Result<()> {
    let emit = EmitMode::from_args(args.emit_env, args.emit_cmd);
    let Some(command) = args.command else {
        Args::command().print_help()?;
        println!();
        return Ok(());
    };
    match command {
        Command::Init => init(&state),
        Command::List { candidate, order } => list(&state, candidate, order),
        Command::Install {
            candidate,
            version,
            local_path,
        } => install(&state, &candidate, version, local_path),
        Command::Uninstall { candidate, version } => uninstall(&state, &candidate, version),
        Command::Use { candidate, version } => use_version(&state, &candidate, version, emit),
        Command::Default { candidate, version } => default_version(&state, &candidate, version),
        Command::Current { candidate } => current(&state, candidate),
        Command::Home { candidate, version } => home(&state, &candidate, version),
        Command::Env { action } => env_cmd(&state, action, emit),
        Command::Offline { action } => offline(&state, action),
        Command::Update => update(&state),
        Command::Upgrade => unsupported(
            "upgrade",
            "Automatic SDK upgrades are not implemented yet. Use `sdk install <candidate> <version>` and `sdk default <candidate> <version>` explicitly.",
        ),
        Command::Selfupdate => unsupported(
            "selfupdate",
            "Self-update is not implemented yet. Download a newer release artifact and run install.ps1 again.",
        ),
        Command::Flush { target } => flush(&state, target.unwrap_or(FlushTarget::All)),
        Command::Config { action } => config(&state, action),
        Command::Version => version(),
        Command::Complete { words } => complete(&state, &words),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EmitMode {
    None,
    PowerShell,
    Cmd,
}

impl EmitMode {
    fn from_args(emit_env: bool, emit_cmd: bool) -> Self {
        if emit_cmd {
            Self::Cmd
        } else if emit_env {
            Self::PowerShell
        } else {
            Self::None
        }
    }
}

fn init(state: &State) -> Result<()> {
    state.init()?;
    println!(
        "Initialized SDKMAN for Windows at {}",
        display_path(&state.root)
    );
    Ok(())
}

fn list(state: &State, candidate: Option<String>, order: Option<Order>) -> Result<()> {
    state.init()?;
    let order = order.unwrap_or(Order::Desc);
    if let Some(ref candidate) = candidate {
        ensure_candidate_exists(state, candidate)?;
    }
    match candidate {
        None => {
            let api = Api::new(state)?;
            println!("Available Candidates");
            for candidate in api.candidates(state.config.offline_mode)? {
                if candidate.description.is_empty() {
                    println!("{}", candidate.name);
                } else {
                    println!("{:<18} {}", candidate.name, candidate.description);
                }
            }
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
            sort_versions_by_vendor_and_version(&mut remote_versions, order);
            let java_table = candidate.eq_ignore_ascii_case("java")
                && remote_versions
                    .iter()
                    .any(|version| version.vendor.is_some());
            if java_table {
                print_java_table_header();
            }
            let mut printed = BTreeSet::new();
            for version in remote_versions {
                let installed_marker = installed.contains(&version.value);
                print_list_version_or_java_row(
                    state,
                    &candidate,
                    &version,
                    installed_marker,
                    current.as_deref(),
                    java_table,
                )?;
                printed.insert(version.value);
            }
            let mut installed_versions_local = installed
                .into_iter()
                .map(Version::local)
                .collect::<Vec<_>>();
            sort_versions_by_vendor_and_version(&mut installed_versions_local, order);
            for version in installed_versions_local {
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

fn sort_versions_by_vendor_and_version(versions: &mut [Version], order: Order) {
    fn parse_semver(s: &str) -> (i64, i64, i64, String) {
        // Split into main part and suffix after first '-'
        let mut parts = s.splitn(2, '-');
        let main = parts.next().unwrap_or("");
        let suffix = parts.next().unwrap_or("").to_string();
        // Extract up to three numeric components from the main part
        let nums = main
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse::<i64>().unwrap_or(0))
            .collect::<Vec<_>>();
        let major = *nums.first().unwrap_or(&0);
        let minor = *nums.get(1).unwrap_or(&0);
        let patch = *nums.get(2).unwrap_or(&0);
        (major, minor, patch, suffix)
    }

    versions.sort_by(|a, b| {
        let sa = a.display_version.as_deref().unwrap_or(&a.value);
        let sb = b.display_version.as_deref().unwrap_or(&b.value);
        let (am, an, ap, asuf) = parse_semver(sa);
        let (bm, bn, bp, bsuf) = parse_semver(sb);

        // Primary: numeric semver comparison (major, minor, patch)
        match am.cmp(&bm).then(an.cmp(&bn)).then(ap.cmp(&bp)) {
            std::cmp::Ordering::Equal => {
                // Tie-breaker: vendor (case-insensitive), then suffix, then full string
                let va = a.vendor.as_deref().unwrap_or("");
                let vb = b.vendor.as_deref().unwrap_or("");
                match va.to_lowercase().cmp(&vb.to_lowercase()) {
                    std::cmp::Ordering::Equal => match asuf.cmp(&bsuf) {
                        std::cmp::Ordering::Equal => sa.cmp(sb),
                        ord => ord,
                    },
                    ord => ord,
                }
            }
            ord => ord,
        }
    });

    if let Order::Desc = order {
        versions.reverse();
    }
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
    let use_marker =
        if installed && installed_version_is_current(state, candidate, &version.value, current)? {
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
        if installed && installed_version_is_current(state, candidate, version, current)? {
            ">"
        } else {
            " "
        };
    println!("{current_marker} {installed_marker} {version}");
    Ok(())
}

fn installed_version_is_current(
    state: &State,
    candidate: &str,
    version: &str,
    current: Option<&Path>,
) -> Result<bool> {
    let Some(current) = current else {
        return Ok(false);
    };
    let Some(record) = state.install_record(candidate, version)? else {
        return Ok(false);
    };
    Ok(paths_match(&record.path, current))
}

fn paths_match(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn install(
    state: &State,
    candidate: &str,
    version: Option<String>,
    local_path: Option<PathBuf>,
) -> Result<()> {
    state.init()?;
    if local_path.is_none() && state.config.offline_mode {
        bail!("install requires network while offline mode is enabled");
    }
    let version = match version {
        Some(version) if local_path.is_some() => version,
        Some(version) => resolve_install_version(state, candidate, &version)?,
        None => select_install_version(state, candidate)?,
    };

    if let Some(local_path) = local_path {
        if !local_path.exists() {
            bail!("local path does not exist: {}", local_path.display());
        }
        let version_dir = state.version_dir(candidate, &version);
        fs::create_dir_all(&version_dir)?;
        let record = InstallRecord {
            candidate: candidate.to_string(),
            version: version.clone(),
            path: fs::canonicalize(local_path)?,
            local: true,
        };
        state.write_record(&record)?;
        println!("Registered {candidate} {version} as local install.");
    } else {
        let api = Api::new(state)?;
        let client = Client::builder().build()?;
        let base_name = format!("{candidate}-{version}");
        println!("Downloading: {candidate} {version}");
        let urls = api.download_url(candidate, &version);
        let archive_path =
            archive::download_with_fallback(&client, &urls, &state.archives_dir(), &base_name)
                .with_context(|| format!("failed to download {candidate} {version}"))?;
        let tmp = TempDir::new_in(state.tmp_dir())?;
        println!("Installing: {candidate} {version}");
        let extract_result = archive::extract(&archive_path, tmp.path());
        let normalized = match extract_result {
            Ok(path) => path,
            Err(e) => {
                let _ = fs::remove_file(&archive_path);
                return Err(e).with_context(|| format!("failed to extract {candidate} {version}"));
            }
        };
        let final_dir = state.version_dir(candidate, &version);
        if let Err(e) = archive::move_normalized(&normalized, &final_dir) {
            let _ = fs::remove_file(&archive_path);
            if final_dir.exists() {
                let _ = fs::remove_dir_all(&final_dir);
            }
            return Err(e);
        }
        state.write_record(&InstallRecord {
            candidate: candidate.to_string(),
            version: version.clone(),
            path: final_dir,
            local: false,
        })?;
    }

    if should_set_default(&state.config)? {
        default_version(state, candidate, Some(version.clone()))?;
    }
    Ok(())
}

fn select_install_version(state: &State, candidate: &str) -> Result<String> {
    let api = Api::new(state)?;
    let versions = api.versions(candidate, state.config.offline_mode)?;
    if versions.is_empty() {
        bail!("no versions available");
    }
    if state.config.auto_answer {
        return Ok(versions[0].value.clone());
    }
    select_version(state, candidate, "all", &versions)
}

fn resolve_install_version(state: &State, candidate: &str, requested: &str) -> Result<String> {
    let api = Api::new(state)?;
    let versions = api.versions(candidate, state.config.offline_mode)?;
    if versions.iter().any(|version| version.value == requested) {
        return Ok(requested.to_string());
    }

    let matches = versions
        .into_iter()
        .filter(|version| version_matches_prefix(version, requested))
        .collect::<Vec<_>>();
    match matches.len() {
        0 => bail!("no {candidate} version matches '{requested}'"),
        1 => Ok(matches[0].value.clone()),
        _ if state.config.auto_answer => Ok(matches[0].value.clone()),
        _ => select_version(state, candidate, requested, &matches),
    }
}

fn version_matches_prefix(version: &Version, prefix: &str) -> bool {
    version.value.starts_with(prefix)
        || version
            .display_version
            .as_deref()
            .is_some_and(|display| display.starts_with(prefix))
}

fn ensure_candidate_exists(state: &State, candidate: &str) -> Result<()> {
    // Prefer installed candidates (works in offline mode too)
    let installed = state.installed_candidates()?;
    if installed.iter().any(|c| c.eq_ignore_ascii_case(candidate)) {
        return Ok(());
    }

    // If offline, we can't query remote metadata
    if state.config.offline_mode {
        bail!("Unknown candidate: {}", candidate);
    }

    let api = Api::new(state)?;
    let remote = api.candidates(state.config.offline_mode)?;
    if remote
        .iter()
        .any(|c| c.name.eq_ignore_ascii_case(candidate))
    {
        Ok(())
    } else {
        bail!("Unknown candidate: {}", candidate);
    }
}

fn select_version(
    state: &State,
    candidate: &str,
    requested: &str,
    versions: &[Version],
) -> Result<String> {
    if io::stderr().is_terminal() && io::stdin().is_terminal() {
        return select_version_interactive(state, candidate, requested, versions);
    }

    println!("Multiple {candidate} versions match '{requested}'");
    println!();
    println!(" {:>3}  {:<18} {:<10} Vendor", "#", "Identifier", "Dist");
    println!(" {}", "-".repeat(58));
    for (index, version) in versions.iter().enumerate() {
        println!(
            " {:>3}  {:<18} {:<10} {}",
            index + 1,
            version.value,
            version.distribution.as_deref().unwrap_or(""),
            version.vendor.as_deref().unwrap_or("")
        );
    }
    println!();
    print!("Select [1-{}] or q to cancel: ", versions.len());
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let answer = answer.trim();
    if answer.eq_ignore_ascii_case("q") || answer.eq_ignore_ascii_case("quit") {
        bail!("selection cancelled");
    }
    let choice = answer
        .parse::<usize>()
        .context("selection must be a number")?;
    if choice == 0 || choice > versions.len() {
        bail!("selection must be between 1 and {}", versions.len());
    }
    Ok(versions[choice - 1].value.clone())
}

fn select_version_interactive(
    state: &State,
    candidate: &str,
    requested: &str,
    versions: &[Version],
) -> Result<String> {
    let mut out = io::stderr();
    let _guard = TerminalMode::enter()?;
    drain_pending_events()?;
    let current = state.active_home(candidate, None).ok().flatten();
    let installed = state
        .installed_versions(candidate)
        .map(|versions| versions.into_iter().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    let context = PickerContext {
        state,
        candidate,
        requested,
        current: current.as_deref(),
        installed: &installed,
    };

    // Work on a local, mutable copy so we can re-sort when the user toggles order.
    let mut versions_vec = versions.to_vec();
    let mut order = Order::Desc;
    sort_versions_by_vendor_and_version(&mut versions_vec, order);

    let mut selected = 0usize;
    let mut last_drawn_selected = usize::MAX;
    let mut last_drawn_order = order;

    loop {
        if selected != last_drawn_selected || order != last_drawn_order {
            draw_version_picker(&mut out, &context, &versions_vec, selected, order)?;
            last_drawn_selected = selected;
            last_drawn_order = order;
        }
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1).min(versions_vec.len() - 1);
                }
                KeyCode::PageUp => {
                    selected = selected.saturating_sub(picker_page_size());
                }
                KeyCode::PageDown => {
                    selected = (selected + picker_page_size()).min(versions_vec.len() - 1);
                }
                KeyCode::Home => selected = 0,
                KeyCode::End => selected = versions_vec.len() - 1,
                KeyCode::Enter => {
                    return Ok(versions_vec[selected].value.clone());
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    bail!("selection cancelled");
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    // Map 1-9 to visible rows
                    let viewport = picker_page_size().min(versions_vec.len());
                    let offset = picker_offset(selected, versions_vec.len(), viewport);
                    let digit = c.to_digit(10).unwrap();
                    if (1..=9).contains(&digit) {
                        let idx = offset + (digit as usize) - 1;
                        if idx < versions_vec.len() {
                            selected = idx;
                        }
                    }
                }
                KeyCode::Char('s') => {
                    // Toggle sort order and re-sort while keeping selection on the same item if possible.
                    order = match order {
                        Order::Desc => Order::Asc,
                        Order::Asc => Order::Desc,
                    };
                    // Remember currently selected identifier
                    let selected_id = versions_vec.get(selected).map(|v| v.value.clone());
                    sort_versions_by_vendor_and_version(&mut versions_vec, order);
                    // Restore selected index to the same identifier if present
                    if let Some(id) = selected_id {
                        selected = versions_vec.iter().position(|v| v.value == id).unwrap_or(0);
                    } else {
                        selected = 0;
                    }
                    // force a redraw on next loop
                    last_drawn_selected = usize::MAX;
                }
                _ => {}
            }
        }
    }
}

fn drain_pending_events() -> Result<()> {
    while event::poll(Duration::from_millis(0))? {
        let _ = event::read()?;
    }
    Ok(())
}

struct PickerContext<'a> {
    state: &'a State,
    candidate: &'a str,
    requested: &'a str,
    current: Option<&'a Path>,
    installed: &'a BTreeSet<String>,
}

fn draw_version_picker(
    out: &mut impl Write,
    context: &PickerContext<'_>,
    versions: &[Version],
    selected: usize,
    order: Order,
) -> Result<()> {
    let viewport = picker_page_size().min(versions.len());
    let offset = picker_offset(selected, versions.len(), viewport);
    let end = (offset + viewport).min(versions.len());
    execute!(
        out,
        cursor::MoveTo(0, 0),
        Clear(ClearType::All),
        Print("\n"),
        Print(" SDKMAN for Windows\n"),
        Print(" ==================\n\n"),
        Print(format!(
            " Select {} version matching '{}'\n\n",
            context.candidate, context.requested
        )),
        Print(format!(
            " Sorted by: Vendor, Version ({})\n",
            match order {
                Order::Desc => "desc (highest first)",
                Order::Asc => "asc (lowest first)",
            }
        )),
        Print(" Shortcuts: Up/Down, PgUp/PgDn, Enter, s toggle sort, 1-9 select, Esc/q.\n\n"),
        Print(format!(
            "   {:<2} {:<18} {:<10} {:<18} \n",
            "", "Identifier", "Dist", "Status"
        )),
        Print(format!("   {}\n", "-".repeat(58)))
    )?;
    for (row, index) in (offset..end).enumerate() {
        let version = &versions[index];
        let scrollbar = scrollbar_glyph(row, viewport, offset, versions.len());
        let status = picker_status(context, version)?;
        if index == selected {
            execute!(out, SetAttribute(Attribute::Reverse), Print(" > "))?;
        } else {
            execute!(out, Print("   "))?;
        }
        execute!(
            out,
            Print(format!(
                "{:<2} {:<18} {:<10} {:<18} {}\n",
                if status.is_current { "*" } else { "" },
                version.value,
                version.distribution.as_deref().unwrap_or(""),
                status.label,
                scrollbar
            ))
        )?;
        if index == selected {
            execute!(out, SetAttribute(Attribute::Reset))?;
        }
    }
    execute!(
        out,
        Print(format!(
            "\n Showing {}-{} of {}. Up/Down, PgUp/PgDn, Enter, Esc/q.",
            offset + 1,
            end,
            versions.len()
        ))
    )?;
    out.flush()?;
    Ok(())
}

struct PickerStatus {
    label: String,
    is_current: bool,
}

fn picker_status(context: &PickerContext<'_>, version: &Version) -> Result<PickerStatus> {
    if installed_version_is_current(
        context.state,
        context.candidate,
        &version.value,
        context.current,
    )? {
        return Ok(PickerStatus {
            label: "current".to_string(),
            is_current: true,
        });
    }
    if context.installed.contains(&version.value) {
        return Ok(PickerStatus {
            label: "installed".to_string(),
            is_current: false,
        });
    }
    Ok(PickerStatus {
        label: version.vendor.clone().unwrap_or_default(),
        is_current: false,
    })
}

fn picker_page_size() -> usize {
    let height = terminal::size()
        .map(|(_, height)| height as usize)
        .unwrap_or(24);
    height.saturating_sub(10).clamp(6, 18)
}

fn picker_offset(selected: usize, total: usize, viewport: usize) -> usize {
    if total <= viewport {
        return 0;
    }
    let half = viewport / 2;
    selected
        .saturating_sub(half)
        .min(total.saturating_sub(viewport))
}

fn scrollbar_glyph(row: usize, viewport: usize, offset: usize, total: usize) -> &'static str {
    if total <= viewport {
        return " ";
    }
    let thumb_size = ((viewport * viewport) / total).clamp(1, viewport);
    let max_thumb_top = viewport - thumb_size;
    let max_offset = total - viewport;
    let thumb_top = (offset * max_thumb_top + max_offset / 2)
        .checked_div(max_offset)
        .unwrap_or(0);
    if row >= thumb_top && row < thumb_top + thumb_size {
        "#"
    } else {
        "|"
    }
}

struct TerminalMode;

impl TerminalMode {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stderr(), EnterAlternateScreen, cursor::Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalMode {
    fn drop(&mut self) {
        let _ = execute!(io::stderr(), cursor::Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

fn should_set_default(config: &Config) -> Result<bool> {
    if config.auto_answer {
        return Ok(true);
    }
    print!("Set as default? (Y/n): ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(answer.trim().is_empty() || answer.trim().eq_ignore_ascii_case("y"))
}

fn uninstall(state: &State, candidate: &str, version: Option<String>) -> Result<()> {
    state.init()?;
    ensure_candidate_exists(state, candidate)?;
    let version = resolve_installed_version(state, candidate, version.as_deref())?;
    let record = state
        .install_record(candidate, &version)?
        .context("version is not installed")?;
    let version_dir = state.version_dir(candidate, &version);
    if record.local {
        fs::remove_file(state.record_path(candidate, &version)).ok();
        if version_dir.exists() {
            fs::remove_dir(version_dir).ok();
        }
        println!("Deregistered local {candidate} {version}.");
    } else {
        fs::remove_dir_all(&version_dir)?;
        println!("Uninstalled {candidate} {version}.");
    }
    let current = state.current_link(candidate);
    if current.exists() && paths_match(&current, &record.path) {
        fslink::remove_linkish(&current).ok();
    }
    shims::regenerate(state)?;
    Ok(())
}

fn use_version(
    state: &State,
    candidate: &str,
    version: Option<String>,
    emit: EmitMode,
) -> Result<()> {
    state.init()?;
    ensure_candidate_exists(state, candidate)?;
    let version = resolve_installed_version(state, candidate, version.as_deref())?;
    let record = state
        .install_record(candidate, &version)?
        .context("version is not installed")?;
    let bin = record.path.join("bin");
    if emit != EmitMode::None {
        let mut set = BTreeMap::new();
        set.insert(
            session_home_var(candidate),
            record.path.display().to_string(),
        );
        set.insert(
            format!("{}_HOME", candidate.to_ascii_uppercase().replace('-', "_")),
            record.path.display().to_string(),
        );
        let update = EnvUpdate {
            set,
            prepend_path: if bin.exists() {
                vec![bin.display().to_string()]
            } else {
                Vec::new()
            },
            message: format!("Using {candidate} version {version} in this shell."),
        };
        emit_update(emit, &update)?;
    } else {
        println!("Use the PowerShell wrapper for session switching: sdk use {candidate} {version}");
        println!("Home: {}", display_path(&record.path));
    }
    Ok(())
}

fn resolve_installed_version(
    state: &State,
    candidate: &str,
    requested: Option<&str>,
) -> Result<String> {
    if let Some(requested) = requested {
        if state.install_record(candidate, requested)?.is_some() {
            return Ok(requested.to_string());
        }
    }

    let versions = state
        .installed_versions(candidate)?
        .into_iter()
        .filter(|version| requested.is_none_or(|requested| version.starts_with(requested)))
        .map(Version::local)
        .collect::<Vec<_>>();

    match versions.len() {
        0 => match requested {
            Some(requested) => bail!("{candidate} {requested} is not installed"),
            None => bail!("no installed {candidate} versions"),
        },
        1 => Ok(versions[0].value.clone()),
        _ if state.config.auto_answer => Ok(versions[0].value.clone()),
        _ => select_version(
            state,
            candidate,
            requested.unwrap_or("installed"),
            &versions,
        ),
    }
}

fn default_version(state: &State, candidate: &str, version: Option<String>) -> Result<()> {
    state.init()?;
    ensure_candidate_exists(state, candidate)?;
    let version = resolve_installed_version(state, candidate, version.as_deref())?;
    let record = state
        .install_record(candidate, &version)?
        .context("version is not installed")?;
    fslink::replace_dir_link(&state.current_link(candidate), &record.path)?;
    shims::regenerate(state)?;
    println!("Default {candidate} version set to {version}.");
    Ok(())
}

fn current(state: &State, candidate: Option<String>) -> Result<()> {
    state.init()?;
    if let Some(candidate) = candidate {
        ensure_candidate_exists(state, &candidate)?;
        match state.active_home(&candidate, None)? {
            Some(home) => println!("Using {candidate} at {}", display_path(&home)),
            None => println!("No {candidate} version is currently in use."),
        }
        return Ok(());
    }
    println!("Using:");
    for candidate in state.installed_candidates()? {
        if let Some(home) = state.active_home(&candidate, None)? {
            println!("{candidate}: {}", display_path(&home));
        }
    }
    Ok(())
}

fn home(state: &State, candidate: &str, version: Option<String>) -> Result<()> {
    state.init()?;
    ensure_candidate_exists(state, candidate)?;
    let home = state
        .active_home(candidate, version.as_deref())?
        .context("version is not installed or active")?;
    println!("{}", display_path(&home));
    Ok(())
}

fn env_cmd(state: &State, action: EnvAction, emit: EmitMode) -> Result<()> {
    state.init()?;
    let rc = env::current_dir()?.join(".sdkmanrc");
    match action {
        EnvAction::Init => {
            if rc.exists() {
                println!(".sdkmanrc already exists");
            } else {
                fs::write(
                    &rc,
                    "# Add candidate versions, for example:\n# java=21.0.4-tem\n",
                )?;
                println!("Created {}", rc.display());
            }
        }
        EnvAction::Clear => {
            if rc.exists() {
                fs::remove_file(&rc)?;
            }
            println!("Removed {}", rc.display());
        }
        EnvAction::Install => {
            let values = envfile::parse(&rc)?;
            let mut update = EnvUpdate {
                set: BTreeMap::new(),
                prepend_path: Vec::new(),
                message: "Applied .sdkmanrc".to_string(),
            };
            for (candidate, version) in values {
                let record = state
                    .install_record(&candidate, &version)?
                    .with_context(|| format!("{candidate} {version} is not installed"))?;
                if emit != EmitMode::None {
                    update.set.insert(
                        session_home_var(&candidate),
                        record.path.display().to_string(),
                    );
                    update.set.insert(
                        format!("{}_HOME", candidate.to_ascii_uppercase().replace('-', "_")),
                        record.path.display().to_string(),
                    );
                    let bin = record.path.join("bin");
                    if bin.exists() {
                        update.prepend_path.push(bin.display().to_string());
                    }
                } else {
                    println!("{candidate}={version} -> {}", display_path(&record.path));
                }
            }
            if emit != EmitMode::None {
                emit_update(emit, &update)?;
            }
        }
    }
    Ok(())
}

fn emit_update(mode: EmitMode, update: &EnvUpdate) -> Result<()> {
    match mode {
        EmitMode::None => {}
        EmitMode::PowerShell => println!("{ENV_JSON_PREFIX}{}", serde_json::to_string(update)?),
        EmitMode::Cmd => {
            for (key, value) in &update.set {
                println!("set \"{}={}\"", key, value);
            }
            for path in update.prepend_path.iter().rev() {
                println!("set \"PATH={};%PATH%\"", path);
            }
            if !update.message.is_empty() {
                println!("echo {}", update.message);
            }
        }
    }
    Ok(())
}

fn offline(state: &State, action: OfflineAction) -> Result<()> {
    state.init()?;
    let mut cfg = state.config.clone();
    cfg.offline_mode = matches!(action, OfflineAction::Enable);
    cfg.write(&state.config_path())?;
    println!(
        "{}",
        if cfg.offline_mode {
            "Forced offline mode enabled."
        } else {
            "Online mode re-enabled!"
        }
    );
    Ok(())
}

fn update(state: &State) -> Result<()> {
    state.init()?;
    if state.config.offline_mode {
        bail!("update requires network while offline mode is enabled");
    }
    Api::new(state)?
        .refresh()
        .context("failed to refresh candidate metadata")?;
    println!("Candidate metadata refreshed.");
    Ok(())
}

fn unsupported(command: &str, guidance: &str) -> Result<()> {
    bail!("sdk {command} is not supported yet. {guidance}")
}

fn flush(state: &State, target: FlushTarget) -> Result<()> {
    state.init()?;
    let targets = match target {
        FlushTarget::Archives => vec![state.archives_dir()],
        FlushTarget::Tmp => vec![state.tmp_dir()],
        FlushTarget::Metadata => vec![state.metadata_dir()],
        FlushTarget::All => vec![state.archives_dir(), state.tmp_dir(), state.metadata_dir()],
    };
    for dir in targets {
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        fs::create_dir_all(&dir)?;
    }
    println!("Flush complete.");
    Ok(())
}

fn config(state: &State, action: Option<ConfigAction>) -> Result<()> {
    state.init()?;
    match action {
        Some(ConfigAction::Set { key, value }) => {
            let mut cfg = state.config.clone();
            cfg.set_key(&key, &value)?;
            cfg.write(&state.config_path())?;
            println!("{key}={value}");
        }
        None => {
            println!("Config: {}", state.config_path().display());
            print!("{}", state.config.to_properties());
        }
    }
    Ok(())
}

fn complete(state: &State, words: &[String]) -> Result<()> {
    for item in completions(state, words) {
        println!("{item}");
    }
    Ok(())
}

fn completions(state: &State, words: &[String]) -> Vec<String> {
    let words = trim_command_name(words);
    let command = words.first().map(String::as_str).unwrap_or_default();
    if words.len() <= 1 {
        return matching(COMMANDS, command);
    }

    let current = words.last().map(String::as_str).unwrap_or_default();
    match command {
        "install" | "i" => complete_install(state, words, current),
        "use" | "default" | "d" | "uninstall" | "rm" | "home" | "h" => {
            complete_installed_version_command(state, words, current)
        }
        "list" | "ls" | "current" | "c" => complete_candidate(state, current, false),
        "flush" => matching(&["archives", "tmp", "metadata", "all"], current),
        "offline" => matching(&["enable", "disable"], current),
        "env" => matching(&["init", "install", "clear"], current),
        "config" => complete_config(words, current),
        _ => Vec::new(),
    }
}

fn trim_command_name(words: &[String]) -> &[String] {
    if words
        .first()
        .and_then(|word| Path::new(word).file_stem())
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case("sdk"))
    {
        &words[1..]
    } else {
        words
    }
}

fn complete_install(state: &State, words: &[String], current: &str) -> Vec<String> {
    match words.len() {
        2 => complete_candidate(state, current, true),
        3 => words
            .get(1)
            .map(|candidate| complete_install_versions(state, candidate, current))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn complete_installed_version_command(
    state: &State,
    words: &[String],
    current: &str,
) -> Vec<String> {
    match words.len() {
        2 => complete_candidate(state, current, false),
        3 => words
            .get(1)
            .map(|candidate| complete_installed_versions(state, candidate, current))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn complete_candidate(state: &State, prefix: &str, include_remote: bool) -> Vec<String> {
    let mut candidates = state.installed_candidates().unwrap_or_default();
    if include_remote && !state.config.offline_mode {
        if let Ok(remote) = Api::new(state).and_then(|api| api.candidates(false)) {
            candidates.extend(remote.into_iter().map(|candidate| candidate.name));
        }
    }
    candidates.sort();
    candidates.dedup();
    matching_owned(candidates, prefix)
}

fn complete_install_versions(state: &State, candidate: &str, prefix: &str) -> Vec<String> {
    Api::new(state)
        .and_then(|api| api.versions(candidate, state.config.offline_mode))
        .map(|versions| {
            matching_owned(
                versions
                    .into_iter()
                    .map(|version| version.value)
                    .collect::<Vec<_>>(),
                prefix,
            )
        })
        .unwrap_or_else(|_| complete_installed_versions(state, candidate, prefix))
}

fn complete_installed_versions(state: &State, candidate: &str, prefix: &str) -> Vec<String> {
    matching_owned(
        state.installed_versions(candidate).unwrap_or_default(),
        prefix,
    )
}

fn complete_config(words: &[String], current: &str) -> Vec<String> {
    match words.len() {
        2 => matching(&["set"], current),
        3 if words.get(1).is_some_and(|word| word == "set") => matching(CONFIG_KEYS, current),
        _ => Vec::new(),
    }
}

fn matching(values: &[&str], prefix: &str) -> Vec<String> {
    matching_owned(
        values.iter().map(|value| (*value).to_string()).collect(),
        prefix,
    )
}

fn matching_owned(mut values: Vec<String>, prefix: &str) -> Vec<String> {
    values.sort();
    values.dedup();
    values
        .into_iter()
        .filter(|value| value.starts_with(prefix))
        .collect()
}

fn version() -> Result<()> {
    println!("SDKMAN for Windows");
    println!("native: {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}

const COMMANDS: &[&str] = &[
    "init",
    "list",
    "ls",
    "install",
    "i",
    "uninstall",
    "rm",
    "use",
    "default",
    "d",
    "current",
    "c",
    "home",
    "h",
    "env",
    "offline",
    "update",
    "upgrade",
    "selfupdate",
    "flush",
    "config",
    "version",
];

const CONFIG_KEYS: &[&str] = &[
    "sdkman_auto_answer",
    "sdkman_insecure_ssl",
    "sdkman_curl_connect_timeout",
    "sdkman_curl_max_time",
    "sdkman_colour_enable",
    "sdkman_debug_mode",
    "sdkman_healthcheck_enable",
    "sdkman_auto_env",
    "sdkman_offline_mode",
];

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn paths_match_after_canonicalization() {
        let root = TempDir::new().unwrap();
        let nested = root.path().join("a").join("b");
        fs::create_dir_all(&nested).unwrap();

        let direct = root.path().join("a").join("b");
        let parent_relative = root.path().join("a").join("..").join("a").join("b");

        assert!(paths_match(&direct, &parent_relative));
    }

    #[test]
    fn partial_install_version_resolves_from_cached_versions() {
        let root = TempDir::new().unwrap();
        fs::create_dir_all(root.path().join("var").join("metadata")).unwrap();
        fs::write(
            root.path()
                .join("var")
                .join("metadata")
                .join("java-versions.txt"),
            "25.0.3-tem,21.0.11-tem",
        )
        .unwrap();
        let state = State {
            root: root.path().to_path_buf(),
            config: Config {
                offline_mode: true,
                ..Config::default()
            },
        };

        let version = resolve_install_version(&state, "java", "25").unwrap();

        assert_eq!(version, "25.0.3-tem");
    }

    #[test]
    fn missing_installed_version_resolves_single_installed_version() {
        let root = TempDir::new().unwrap();
        fs::create_dir_all(root.path().join("candidates").join("java").join("21-local")).unwrap();
        let state = State {
            root: root.path().to_path_buf(),
            config: Config::default(),
        };

        let version = resolve_installed_version(&state, "java", None).unwrap();

        assert_eq!(version, "21-local");
    }

    #[test]
    fn missing_installed_version_fails_when_none_are_installed() {
        let root = TempDir::new().unwrap();
        let state = State {
            root: root.path().to_path_buf(),
            config: Config::default(),
        };

        let error = resolve_installed_version(&state, "java", None).unwrap_err();

        assert!(error.to_string().contains("no installed java versions"));
    }
}
