use crossterm::{
    cursor,
    event::{self, Event},
    execute, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};
use std::time::{Duration, Instant};

// Muted color palette
const C_WARM: Color = Color::Rgb {
    r: 138,
    g: 117,
    b: 96,
};
const C_STEEL: Color = Color::Rgb {
    r: 90,
    g: 101,
    b: 119,
};
const C_SILVER: Color = Color::Rgb {
    r: 160,
    g: 168,
    b: 183,
};
const C_SAGE: Color = Color::Rgb {
    r: 90,
    g: 158,
    b: 111,
};
const C_RUST: Color = Color::Rgb {
    r: 158,
    g: 90,
    b: 90,
};
const C_DIM: Color = Color::Rgb {
    r: 70,
    g: 78,
    b: 90,
};

const ASCII_LOGO: &[&str] = &[
    r"        _ _          _                          ",
    r" __   _(_) |__   ___| |_ _ __ __ _  ___ ___ _ __ ",
    r" \ \ / / | '_ \ / _ \ __| '__/ _` |/ __/ _ \ '__|",
    r"  \ V /| | |_) |  __/ |_| | | (_| | (_|  __/ |   ",
    r"   \_/ |_|_.__/ \___|\__|_|  \__,_|\___\___|_|   ",
];

const TAGLINE: &str = "trace. replay. rewind.";

fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// Returns true if a keypress was detected (so we can bail early).
fn key_pressed() -> bool {
    if event::poll(Duration::from_millis(0)).unwrap_or(false) {
        if let Ok(Event::Key(_)) = event::read() {
            return true;
        }
    }
    false
}

pub fn play_splash() -> anyhow::Result<()> {
    let mut stdout = io::stdout();

    terminal::enable_raw_mode()?;
    execute!(
        stdout,
        EnterAlternateScreen,
        cursor::Hide,
        Clear(ClearType::All)
    )?;

    let result = run_animation(&mut stdout);

    // Always restore terminal state, even if animation errored or user pressed key
    execute!(stdout, LeaveAlternateScreen, cursor::Show)?;
    terminal::disable_raw_mode()?;

    result
}

fn run_animation(stdout: &mut io::Stdout) -> anyhow::Result<()> {
    let (cols, rows) = terminal::size()?;
    let cols = cols as usize;
    let rows = rows as usize;

    // Bail out early if the terminal is too small for the animation.
    if cols < 10 || rows < 10 {
        return Ok(());
    }

    // ── Logo geometry ────────────────────────────────────────────────────────
    let logo_width = ASCII_LOGO.iter().map(|l| l.len()).max().unwrap_or(50);
    let logo_height = ASCII_LOGO.len(); // 5 lines
    let logo_col = cols.saturating_sub(logo_width) / 2;
    let logo_row = rows.saturating_sub(logo_height + 8) / 2;

    // ─────────────────────────────────────────────────────────────────────────
    // Phase 1 (0 – 0.8 s): logo types in character by character
    // We reveal one column of characters per frame (~16ms per column step).
    // ─────────────────────────────────────────────────────────────────────────
    let phase1_start = Instant::now();
    let phase1_dur = Duration::from_millis(800);

    // Pre-compute max visible columns at each step
    let total_cols = logo_width;
    let frame_delay_p1 = 800u64 / (total_cols as u64).max(1);

    for reveal_up_to in 0..=total_cols {
        if key_pressed() {
            return Ok(());
        }

        // Draw each logo line up to the revealed column
        for (li, line) in ASCII_LOGO.iter().enumerate() {
            let visible: String = line.chars().take(reveal_up_to).collect();
            queue!(
                stdout,
                cursor::MoveTo(logo_col as u16, (logo_row + li) as u16),
                SetForegroundColor(C_WARM),
                Print(&visible),
            )?;
        }
        stdout.flush()?;
        sleep_ms(frame_delay_p1.max(8));

        // Abort phase early if already past deadline
        if phase1_start.elapsed() > phase1_dur {
            break;
        }
    }

    // Ensure full logo is visible before moving on
    for (li, line) in ASCII_LOGO.iter().enumerate() {
        queue!(
            stdout,
            cursor::MoveTo(logo_col as u16, (logo_row + li) as u16),
            SetForegroundColor(C_WARM),
            Print(line),
        )?;
    }
    stdout.flush()?;

    // ─────────────────────────────────────────────────────────────────────────
    // Phase 2 (0.8 – 1.6 s): 5 timeline tracks materialise beneath the logo
    // ─────────────────────────────────────────────────────────────────────────
    let track_colors = [C_STEEL, C_SILVER, C_SAGE, C_RUST, C_DIM];
    let track_y_base = logo_row + logo_height + 2;
    let track_width = cols.min(logo_width + 10);
    let track_x = cols.saturating_sub(track_width) / 2;

    // Each track fills in a wave: ░ → ▓ → █ across its width over 160ms.
    // Tracks are staggered by 80ms.
    let stagger_ms = 80u64;
    let track_fill_ms = 160u64;

    let phase2_total_ms = stagger_ms * (track_colors.len() as u64 - 1) + track_fill_ms + 200;
    let phase2_start = Instant::now();

    loop {
        if key_pressed() {
            return Ok(());
        }
        let now_ms = phase2_start.elapsed().as_millis() as u64;

        for (ti, &color) in track_colors.iter().enumerate() {
            let track_start_ms = stagger_ms * ti as u64;
            let row = track_y_base + ti;

            if now_ms < track_start_ms {
                continue;
            }
            let elapsed = now_ms.saturating_sub(track_start_ms);
            let progress = (elapsed as f64 / track_fill_ms as f64).min(1.0);

            let filled = ((progress * track_width as f64) as usize).min(track_width);

            let mut track_str = String::with_capacity(track_width);
            for ci in 0..track_width {
                if ci < filled {
                    // Wave: last few chars show lighter blocks
                    let wave_pos = filled.saturating_sub(ci);
                    let ch = if wave_pos < 3 {
                        '░'
                    } else if wave_pos < 6 {
                        '▓'
                    } else {
                        '█'
                    };
                    track_str.push(ch);
                } else {
                    track_str.push(' ');
                }
            }

            queue!(
                stdout,
                cursor::MoveTo(track_x as u16, row as u16),
                SetForegroundColor(color),
                Print(&track_str),
            )?;
        }
        stdout.flush()?;
        sleep_ms(16);

        if phase2_start.elapsed().as_millis() as u64 >= phase2_total_ms {
            break;
        }
    }

    // Freeze tracks as solid blocks
    for (ti, &color) in track_colors.iter().enumerate() {
        let row = track_y_base + ti;
        let solid: String = "█".repeat(track_width);
        queue!(
            stdout,
            cursor::MoveTo(track_x as u16, row as u16),
            SetForegroundColor(color),
            Print(&solid),
        )?;
    }
    stdout.flush()?;

    // ─────────────────────────────────────────────────────────────────────────
    // Phase 3 (1.6 – 2.0 s): playhead sweeps left → right across all tracks
    // ─────────────────────────────────────────────────────────────────────────
    let phase3_dur = 400u64;
    let phase3_start = Instant::now();

    loop {
        if key_pressed() {
            return Ok(());
        }
        let elapsed_ms = phase3_start.elapsed().as_millis() as u64;
        let progress = (elapsed_ms as f64 / phase3_dur as f64).min(1.0);
        let head_x = track_x + (progress * track_width as f64) as usize;
        let head_x = head_x.min(track_x + track_width.saturating_sub(1));

        // Draw brightened blocks behind playhead + playhead line
        for (ti, &color) in track_colors.iter().enumerate() {
            let row = track_y_base + ti;

            // Brightened region (already passed by playhead)
            let brightened: String = "█".repeat(head_x.saturating_sub(track_x));
            queue!(
                stdout,
                cursor::MoveTo(track_x as u16, row as u16),
                SetForegroundColor(color),
                Print(&brightened),
            )?;

            // Playhead character
            queue!(
                stdout,
                cursor::MoveTo(head_x as u16, row as u16),
                SetForegroundColor(C_SILVER),
                Print("│"),
            )?;
        }
        stdout.flush()?;
        sleep_ms(16);

        if progress >= 1.0 {
            break;
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Phase 4 (2.0 – 2.5 s): tagline fades in, then brief hold + clear
    // ─────────────────────────────────────────────────────────────────────────
    let tagline_row = track_y_base + track_colors.len() + 2;
    let tagline_col = cols.saturating_sub(TAGLINE.len()) / 2;

    // Reveal tagline character by character
    let tagline_reveal_ms = 250u64;
    let chars: Vec<char> = TAGLINE.chars().collect();
    let char_delay = tagline_reveal_ms / chars.len() as u64;

    for (i, _) in chars.iter().enumerate() {
        if key_pressed() {
            return Ok(());
        }
        let visible: String = chars[..=i].iter().collect();
        queue!(
            stdout,
            cursor::MoveTo(tagline_col as u16, tagline_row as u16),
            SetForegroundColor(C_DIM),
            Print(&visible),
        )?;
        stdout.flush()?;
        sleep_ms(char_delay);
    }

    // Hold for ~200ms then clear
    sleep_ms(200);
    execute!(stdout, Clear(ClearType::All), ResetColor)?;

    Ok(())
}
