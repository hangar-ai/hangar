//! What a button or a modal acts on, written in its `custom_id` (100
//! characters at most: a pane's id is 36) and read back when it is clicked.

/// One thing the person can click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Click {
    /// Allow or deny the permission pane `pane` waits on.
    Permit { pane: String, allow: bool },
    /// Option `option` of the one question the pane waits on.
    Choose { pane: String, option: usize },
    /// The person's own answer to the one question (a modal).
    Other { pane: String },
    /// The modal of several questions.
    AnswerAll { pane: String },
    /// Approve the plan, or keep planning (a modal).
    Plan { pane: String, approve: bool },
    /// Land a delivery, or send it back (a modal).
    Deliver {
        ws: String,
        task: String,
        land: bool,
    },
    /// Open the worker's thread.
    Thread { pane: String },
    /// Stop the agent's turn.
    Stop { pane: String },
}

fn flag(yes: bool) -> &'static str {
    if yes { "y" } else { "n" }
}

impl Click {
    pub fn id(&self) -> String {
        match self {
            Click::Permit { pane, allow } => format!("p:{pane}:{}", flag(*allow)),
            Click::Choose { pane, option } => format!("c:{pane}:{option}"),
            Click::Other { pane } => format!("o:{pane}"),
            Click::AnswerAll { pane } => format!("a:{pane}"),
            Click::Plan { pane, approve } => format!("l:{pane}:{}", flag(*approve)),
            Click::Deliver { ws, task, land } => format!("d:{}:{ws}:{task}", flag(*land)),
            Click::Thread { pane } => format!("t:{pane}"),
            Click::Stop { pane } => format!("s:{pane}"),
        }
    }

    pub fn parse(id: &str) -> Option<Click> {
        let (kind, rest) = id.split_once(':')?;
        let yes = |flag: &str| match flag {
            "y" => Some(true),
            "n" => Some(false),
            _ => None,
        };
        let pane = || (!rest.is_empty()).then(|| rest.to_string());
        Some(match kind {
            "p" => {
                let (pane, allow) = rest.rsplit_once(':')?;
                Click::Permit {
                    pane: pane.to_string(),
                    allow: yes(allow)?,
                }
            }
            "c" => {
                let (pane, option) = rest.rsplit_once(':')?;
                Click::Choose {
                    pane: pane.to_string(),
                    option: option.parse().ok()?,
                }
            }
            "o" => Click::Other { pane: pane()? },
            "a" => Click::AnswerAll { pane: pane()? },
            "l" => {
                let (pane, approve) = rest.rsplit_once(':')?;
                Click::Plan {
                    pane: pane.to_string(),
                    approve: yes(approve)?,
                }
            }
            "d" => {
                let mut parts = rest.splitn(3, ':');
                let land = yes(parts.next()?)?;
                let ws = parts.next()?.to_string();
                let task = parts.next()?.to_string();
                if ws.is_empty() || task.is_empty() {
                    return None;
                }
                Click::Deliver { ws, task, land }
            }
            "t" => Click::Thread { pane: pane()? },
            "s" => Click::Stop { pane: pane()? },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_click_reads_back_as_written() {
        let pane = "0d2f6a51-9c3e-4b8a-a1f2-3e4d5c6b7a89".to_string();
        let clicks = [
            Click::Permit {
                pane: pane.clone(),
                allow: true,
            },
            Click::Permit {
                pane: pane.clone(),
                allow: false,
            },
            Click::Choose {
                pane: pane.clone(),
                option: 3,
            },
            Click::Other { pane: pane.clone() },
            Click::AnswerAll { pane: pane.clone() },
            Click::Plan {
                pane: pane.clone(),
                approve: false,
            },
            Click::Deliver {
                ws: pane.clone(),
                task: "t:12".into(),
                land: true,
            },
            Click::Thread { pane: pane.clone() },
            Click::Stop { pane: pane.clone() },
        ];
        for click in clicks {
            let id = click.id();
            assert!(id.len() <= 100, "{id}");
            assert_eq!(Click::parse(&id), Some(click));
        }
        assert_eq!(Click::parse("x:1"), None);
        assert_eq!(Click::parse("p:1:maybe"), None);
        assert_eq!(Click::parse("t:"), None);
    }
}
