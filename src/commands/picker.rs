use anyhow::{bail, Context, Result};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::{Attribute, Print, SetAttribute},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    collections::BTreeSet,
    io::{self, IsTerminal, Write},
    path::Path,
    time::Duration,
};

use crate::{api::Version, cli::Order, state::State};

pub(super) fn select_version(
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
    for (i, version) in versions.iter().enumerate() {
        println!(
            " {:>3}  {:<18} {:<10} {}",
            i + 1,
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
        .map(|vs| vs.into_iter().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    let ctx = PickerContext {
        state,
        candidate,
        requested,
        current: current.as_deref(),
        installed: &installed,
    };

    let mut versions_vec = versions.to_vec();
    let mut order = Order::Desc;
    super::sort_versions_by_vendor_and_version(&mut versions_vec, order);

    let mut selected = 0usize;
    let mut last_drawn_selected = usize::MAX;
    let mut last_drawn_order = order;

    loop {
        if selected != last_drawn_selected || order != last_drawn_order {
            draw_picker(&mut out, &ctx, &versions_vec, selected, order)?;
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
                    selected = selected.saturating_sub(page_size());
                }
                KeyCode::PageDown => {
                    selected = (selected + page_size()).min(versions_vec.len() - 1);
                }
                KeyCode::Home => selected = 0,
                KeyCode::End => selected = versions_vec.len() - 1,
                KeyCode::Enter => return Ok(versions_vec[selected].value.clone()),
                KeyCode::Esc | KeyCode::Char('q') => bail!("selection cancelled"),
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    let viewport = page_size().min(versions_vec.len());
                    let offset = scroll_offset(selected, versions_vec.len(), viewport);
                    let digit = c.to_digit(10).unwrap() as usize;
                    if (1..=9).contains(&digit) {
                        let idx = offset + digit - 1;
                        if idx < versions_vec.len() {
                            selected = idx;
                        }
                    }
                }
                KeyCode::Char('s') => {
                    order = match order {
                        Order::Desc => Order::Asc,
                        Order::Asc => Order::Desc,
                    };
                    let selected_id = versions_vec.get(selected).map(|v| v.value.clone());
                    super::sort_versions_by_vendor_and_version(&mut versions_vec, order);
                    selected = selected_id
                        .and_then(|id| versions_vec.iter().position(|v| v.value == id))
                        .unwrap_or(0);
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

struct PickerStatus {
    label: String,
    is_current: bool,
}

fn picker_status(ctx: &PickerContext<'_>, version: &Version) -> Result<PickerStatus> {
    if super::installed_version_is_current(ctx.state, ctx.candidate, &version.value, ctx.current)? {
        return Ok(PickerStatus {
            label: "current".to_string(),
            is_current: true,
        });
    }
    if ctx.installed.contains(&version.value) {
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

fn draw_picker(
    out: &mut impl Write,
    ctx: &PickerContext<'_>,
    versions: &[Version],
    selected: usize,
    order: Order,
) -> Result<()> {
    let viewport = page_size().min(versions.len());
    let offset = scroll_offset(selected, versions.len(), viewport);
    let end = (offset + viewport).min(versions.len());

    execute!(
        out,
        cursor::MoveTo(0, 0),
        Clear(ClearType::All),
        Print("\n SDKMAN for Windows\n ==================\n\n"),
        Print(format!(
            " Select {} version matching '{}'\n\n",
            ctx.candidate, ctx.requested
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
            "   {:<2} {:<18} {:<10} {:<18} \n   {}\n",
            "",
            "Identifier",
            "Dist",
            "Status",
            "-".repeat(58)
        )),
    )?;

    for (row, index) in (offset..end).enumerate() {
        let version = &versions[index];
        let scrollbar = scrollbar_glyph(row, viewport, offset, versions.len());
        let status = picker_status(ctx, version)?;
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

fn page_size() -> usize {
    let height = terminal::size().map(|(_, h)| h as usize).unwrap_or(24);
    height.saturating_sub(10).clamp(6, 18)
}

fn scroll_offset(selected: usize, total: usize, viewport: usize) -> usize {
    if total <= viewport {
        return 0;
    }
    selected
        .saturating_sub(viewport / 2)
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
