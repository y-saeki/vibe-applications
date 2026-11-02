//! Reading the processes on macOS, from what `ps` and `nettop` print.
//!
//! `/bin/ps` is used rather than libproc because it is set-user-ID root: it
//! reads the CPU time of every process, root's and other users' included,
//! which `proc_pid_rusage` refuses an ordinary user. The parsing is plain
//! text handling and is tested on every platform; only `read` (macOS only)
//! runs the commands.

use crate::sample::{Sample, Samples};

/// The arguments for `/bin/ps`: every process, with its ID, CPU time,
/// resident memory in kilobytes and executable, without a header. Each
/// column is a separate `-o`, as an `=` takes the rest of its argument as
/// the heading.
pub const PS_ARGS: [&str; 9] = [
    "-A", "-o", "pid=", "-o", "time=", "-o", "rss=", "-o", "comm=",
];

/// The arguments for `/usr/bin/nettop`: one reading of the bytes each
/// process has received and sent, as CSV.
pub const NETTOP_ARGS: [&str; 5] = ["-P", "-L", "1", "-J", "bytes_in,bytes_out"];

/// Parses `ps`'s output (see [`PS_ARGS`]). A line that does not parse is
/// skipped.
pub fn parse_ps(output: &str) -> Samples {
    output.lines().filter_map(parse_ps_line).collect()
}

fn parse_ps_line(line: &str) -> Option<(u32, Sample)> {
    let mut rest = line.trim_start();
    let mut field = || {
        let end = rest.find(char::is_whitespace)?;
        let (word, tail) = rest.split_at(end);
        rest = tail.trim_start();
        Some(word)
    };
    let pid = field()?.parse().ok()?;
    let time = parse_time(field()?)?;
    let rss: u64 = field()?.parse().ok()?;
    let command = rest.trim_end();
    if command.is_empty() {
        return None;
    }
    Some((pid, Sample::new(process_name(command), time, rss * 1024)))
}

/// The executable's file name: `ps` prints the path it was started by.
fn process_name(command: &str) -> &str {
    command
        .rsplit('/')
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or(command)
}

/// Parses `ps`'s CPU time, `[[dd-]hh:]mm:ss.ss`, into seconds. The minutes
/// may run past 59 when there are no hours.
pub fn parse_time(text: &str) -> Option<f64> {
    let (days, clock) = match text.split_once('-') {
        Some((d, c)) => (d.parse::<u64>().ok()?, c),
        None => (0, text),
    };
    let mut seconds = 0.0;
    let parts: Vec<&str> = clock.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    for (i, part) in parts.iter().enumerate() {
        let last = i == parts.len() - 1;
        let value: f64 = if last {
            part.replace(',', ".").parse().ok()?
        } else {
            part.parse::<u64>().ok()? as f64
        };
        if !value.is_finite() || value < 0.0 {
            return None;
        }
        seconds = seconds * 60.0 + value;
    }
    Some(days as f64 * 86_400.0 + seconds)
}

/// Adds `nettop`'s byte counts (see [`NETTOP_ARGS`]) to the processes in
/// `samples`. Its lines are `name.pid,bytes_in,bytes_out,`; the name may
/// itself hold dots and commas, so the line is read from the right.
pub fn add_nettop(samples: &mut Samples, output: &str) {
    for line in output.lines() {
        let line = line.trim_end().trim_end_matches(',');
        let mut fields = line.rsplitn(3, ',');
        let (Some(out), Some(r#in), Some(process)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let Some(pid) = process
            .rsplit('.')
            .next()
            .and_then(|p| p.parse::<u32>().ok())
        else {
            continue;
        };
        let (Ok(r#in), Ok(out)) = (r#in.trim().parse::<u64>(), out.trim().parse::<u64>()) else {
            continue;
        };
        if let Some(sample) = samples.get_mut(&pid) {
            // A process can be listed once per interface or flow kind.
            sample.net_in = Some(sample.net_in.unwrap_or(0) + r#in);
            sample.net_out = Some(sample.net_out.unwrap_or(0) + out);
        }
    }
}

/// Reads every process, and with `network` the bytes each has moved.
/// `None` when `ps` cannot be run.
#[cfg(target_os = "macos")]
pub fn read(network: bool) -> Option<Samples> {
    let mut samples = parse_ps(&run("/bin/ps", &PS_ARGS)?);
    if network {
        if let Some(output) = run("/usr/bin/nettop", &NETTOP_ARGS) {
            add_nettop(&mut samples, &output);
        }
    }
    Some(samples)
}

#[cfg(target_os = "macos")]
fn run(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        // A decimal point, whatever the user's region uses.
        .env("LANG", "en_US.UTF-8")
        .env("LC_ALL", "en_US.UTF-8")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_time_in_every_form_ps_writes() {
        assert_eq!(parse_time("0:00.00"), Some(0.0));
        assert_eq!(parse_time("1:02.50"), Some(62.5));
        assert_eq!(parse_time("125:00.00"), Some(7500.0));
        assert_eq!(parse_time("1:00:01.00"), Some(3601.0));
        assert_eq!(parse_time("2-01:00:00.00"), Some(2.0 * 86_400.0 + 3600.0));
        assert_eq!(parse_time("0:01,25"), Some(1.25));
        assert_eq!(parse_time("1.00"), None);
        assert_eq!(parse_time("a:00.00"), None);
        assert_eq!(parse_time(""), None);
    }

    #[test]
    fn reads_every_line_of_ps() {
        let out = "\
    1   2:03.45  12800 /sbin/launchd
  412   0:00.10   2048 /Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Helper (Renderer).app/Contents/MacOS/Google Chrome Helper (Renderer)
  500   0:00.00      0 -zsh
  bad line
  600   0:00.00    100
";
        let s = parse_ps(out);
        assert_eq!(s.len(), 3);
        assert_eq!(s[&1], Sample::new("launchd", 123.45, 12800 * 1024));
        assert_eq!(s[&412].name, "Google Chrome Helper (Renderer)");
        assert_eq!(s[&412].memory, 2048 * 1024);
        assert_eq!(s[&500].name, "-zsh");
    }

    #[test]
    fn adds_nettop_bytes_to_the_processes_ps_listed() {
        let mut s = parse_ps("  10 0:00.00 1 /usr/bin/a\n  20 0:00.00 1 /usr/bin/b\n");
        let out = "\
,bytes_in,bytes_out,
a.b,c.10,1200,300,
b.20,5,6,
b.20,1,1,
ghost.99,1,1,
kernel_task.0,9,9,
garbage
";
        add_nettop(&mut s, out);
        assert_eq!((s[&10].net_in, s[&10].net_out), (Some(1200), Some(300)));
        assert_eq!((s[&20].net_in, s[&20].net_out), (Some(6), Some(7)));
        assert_eq!(s.len(), 2);
    }
}
