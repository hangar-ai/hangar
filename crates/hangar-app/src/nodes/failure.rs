//! Why a machine could not be reached, in words: one short phrase for each
//! thing that can fail, made to follow "offline:" on the pane's card, in a
//! tooltip and in the settings' row, with the cause the tool gave in
//! brackets when it is short. What the tool said in full is kept apart
//! ([`detail`]), for whoever wants to read it.

use hangar_i18n::{Key, Translator, args};

use super::model::{Failure, Step};

/// The longest cause set in a headline; a longer one is only in the detail.
const CAUSE_MAX: usize = 60;

/// What failed, without the tool's own words.
fn what(tr: Translator, failure: &Failure) -> String {
    let arch = |key: Key, arch: &str| tr.t_with(key, &args!("arch" => arch)).into_owned();
    match failure {
        Failure::Ssh { step, .. } => tr
            .t(match step {
                Step::Probe => Key::NodesFailSshProbe,
                Step::Install => Key::NodesFailSshInstall,
                Step::Start => Key::NodesFailSshStart,
                Step::Config => Key::NodesFailSshConfig,
                Step::Tunnel => Key::NodesFailSshTunnel,
                Step::Handshake => Key::NodesFailSshHandshake,
            })
            .into_owned(),
        Failure::NoBinary { arch: found, .. } => arch(Key::NodesFailNoBinary, found),
        Failure::UnsupportedArch(found) => arch(Key::NodesFailUnsupportedArch, found),
        Failure::NodeJson(_) => tr.t(Key::NodesFailNodeJson).into_owned(),
        Failure::Tunnel(_) => tr.t(Key::NodesFailTunnel).into_owned(),
        Failure::Api(_) => tr.t(Key::NodesFailApi).into_owned(),
        Failure::Local(detail) => one_line(detail),
    }
}

/// What the tool said, when the failure carries it.
fn said(failure: &Failure) -> Option<&str> {
    match failure {
        Failure::Ssh { detail, .. }
        | Failure::NodeJson(detail)
        | Failure::Tunnel(detail)
        | Failure::Api(detail) => Some(detail.as_str()),
        Failure::NoBinary { .. } | Failure::UnsupportedArch(_) | Failure::Local(_) => None,
    }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The cause in what a tool said: the end of its last line, after the
/// names of the tool and of what it was doing ("ssh: connect to host vps
/// port 22: Connection timed out" gives "Connection timed out"). `None`
/// when there is none short enough to sit in a headline.
pub fn cause(said: &str) -> Option<String> {
    let line = said.lines().rev().find(|line| !line.trim().is_empty())?;
    let line = line.trim().trim_end_matches('.');
    let end = match line.rsplit_once(": ") {
        Some((_, end)) if end.trim().chars().count() >= 3 => end.trim(),
        _ => line,
    };
    (!end.is_empty() && end.chars().count() <= CAUSE_MAX).then(|| end.to_owned())
}

/// The failure in the person's language, short: "unreachable over SSH
/// (Connection timed out)".
pub fn words(tr: Translator, failure: &Failure) -> String {
    let what = what(tr, failure);
    match said(failure).and_then(cause) {
        Some(cause) => tr
            .t_with(
                Key::NodesFailBecause,
                &args!("what" => what, "cause" => cause),
            )
            .into_owned(),
        None => what,
    }
}

/// What there is to read beyond [`words`]: all the tool said when the
/// headline holds less, or what to do about a build that is missing.
/// Untranslated where it is a tool's.
pub fn detail(tr: Translator, failure: &Failure) -> Option<String> {
    if let Failure::NoBinary { file, dir, .. } = failure {
        return Some(
            tr.t_with(
                Key::NodesFailNoBinaryHelp,
                &args!("file" => file.as_str(), "dir" => dir.as_str()),
            )
            .into_owned(),
        );
    }
    let said = one_line(said(failure)?);
    let shown = cause(&said);
    (!said.is_empty() && shown.as_deref() != Some(said.trim_end_matches('.'))).then_some(said)
}

#[cfg(test)]
mod tests {
    use hangar_i18n::Locale;

    use super::*;

    fn ssh(step: Step, detail: &str) -> Failure {
        Failure::Ssh {
            step,
            detail: detail.into(),
        }
    }

    #[test]
    fn each_step_that_fails_has_its_own_words() {
        let en = Translator::new(Locale::En);
        let fr = Translator::new(Locale::Fr);
        let timed_out = "ssh: connect to host 203.0.113.9 port 22: Connection timed out";
        let cases = [
            (
                ssh(Step::Probe, timed_out),
                "unreachable over SSH (Connection timed out)",
                "injoignable en SSH (Connection timed out)",
            ),
            (
                ssh(
                    Step::Install,
                    "scp: /home/me/.hangar: No space left on device",
                ),
                "hangar-node could not be installed (No space left on device)",
                "hangar-node n’a pas pu être installé (No space left on device)",
            ),
            (
                ssh(Step::Start, ""),
                "hangar-node did not start",
                "hangar-node n’a pas démarré",
            ),
            (
                ssh(Step::Config, "cat: node.json: Permission denied"),
                "its address could not be read (Permission denied)",
                "son adresse n’a pas pu être lue (Permission denied)",
            ),
            (
                ssh(Step::Tunnel, "bind: Address already in use"),
                "the tunnel could not be opened (Address already in use)",
                "le tunnel n’a pas pu être ouvert (Address already in use)",
            ),
            (
                ssh(Step::Handshake, "Permission denied (publickey)."),
                "hangar-node did not say who it is (Permission denied (publickey))",
                "hangar-node ne s’est pas identifié (Permission denied (publickey))",
            ),
            (
                Failure::NoBinary {
                    arch: "riscv64".into(),
                    file: "hangar-node-riscv64".into(),
                    dir: "/opt/hangar".into(),
                },
                "no hangar-node build for riscv64",
                "aucun hangar-node pour riscv64",
            ),
            (
                Failure::UnsupportedArch("Darwin arm64".into()),
                "it runs Darwin arm64, not Linux x86_64 or arm64",
                "elle tourne sur Darwin arm64, pas sur Linux x86_64 ou arm64",
            ),
            (
                Failure::NodeJson("expected value at line 1".into()),
                "its node.json could not be read (expected value at line 1)",
                "son node.json est illisible (expected value at line 1)",
            ),
            (
                Failure::Tunnel("Connection reset".into()),
                "the tunnel closed (Connection reset)",
                "le tunnel s’est fermé (Connection reset)",
            ),
            (
                Failure::Api("HTTP 502".into()),
                "hangar-node did not answer through the tunnel (HTTP 502)",
                "hangar-node n’a pas répondu par le tunnel (HTTP 502)",
            ),
            (
                Failure::Local("ssh was not found on PATH".into()),
                "ssh was not found on PATH",
                "ssh was not found on PATH",
            ),
        ];
        for (failure, english, french) in cases {
            assert_eq!(words(en, &failure), english);
            assert_eq!(words(fr, &failure), french);
            // Short enough for the pane's card and the settings' row.
            for said in [words(en, &failure), words(fr, &failure)] {
                assert!(said.chars().count() <= 72, "{said}");
                assert!(!said.contains('…'), "no step's ellipsis in it: {said}");
            }
        }
    }

    #[test]
    fn the_cause_is_the_end_of_what_the_tool_said() {
        assert_eq!(
            cause("ssh: connect to host vps port 22: Connection timed out").as_deref(),
            Some("Connection timed out")
        );
        assert_eq!(
            cause("Warning: Permanently added 'vps' to the list of known hosts.\nme@vps: Permission denied (publickey).\n").as_deref(),
            Some("Permission denied (publickey)"),
            "the last line, without its full stop"
        );
        assert_eq!(
            cause("Connection reset").as_deref(),
            Some("Connection reset")
        );
        assert_eq!(cause("  \n"), None);
        let long = format!("ssh: {}", "x".repeat(80));
        assert_eq!(cause(&long), None, "too long for a headline");
    }

    #[test]
    fn what_the_tool_said_in_full_is_kept_apart() {
        let en = Translator::new(Locale::En);
        let raw = "ssh: connect to host vps port 22: Connection timed out";
        assert_eq!(detail(en, &ssh(Step::Probe, raw)).as_deref(), Some(raw));
        // Nothing more to read when the headline holds all of it.
        assert_eq!(
            detail(en, &Failure::Tunnel("Connection reset".into())),
            None
        );
        assert_eq!(detail(en, &ssh(Step::Start, "")), None);
        assert_eq!(detail(en, &Failure::Local("no free port".into())), None);
        // A cause too long for the headline is read here.
        let long = format!("ssh: {}", "x".repeat(80));
        assert_eq!(words(en, &ssh(Step::Probe, &long)), "unreachable over SSH");
        assert_eq!(detail(en, &ssh(Step::Probe, &long)), Some(long));
        let missing = Failure::NoBinary {
            arch: "riscv64".into(),
            file: "hangar-node-riscv64".into(),
            dir: "/opt/hangar".into(),
        };
        assert_eq!(
            detail(en, &missing).as_deref(),
            Some(
                "Set HANGAR_NODE_BINARY or put hangar-node-riscv64 beside hangar-ai (/opt/hangar)."
            )
        );
    }
}
