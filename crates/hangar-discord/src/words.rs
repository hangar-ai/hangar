//! What the bot says, in the person's language (the settings' `locale`):
//! French or English, as Hangar speaks.

use hangar_i18n::Locale;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Words {
    fr: bool,
}

impl Words {
    pub fn of(locale: Option<Locale>) -> Words {
        Words {
            fr: locale.unwrap_or_else(Locale::system) == Locale::Fr,
        }
    }

    fn pick(&self, fr: &'static str, en: &'static str) -> &'static str {
        if self.fr { fr } else { en }
    }

    pub fn working(&self) -> &'static str {
        self.pick("travaille", "is working")
    }
    pub fn stop(&self) -> &'static str {
        self.pick("Arrêter", "Stop")
    }
    pub fn actions(&self, n: usize, minutes: u64) -> String {
        if self.fr {
            format!("{n} action{} · {minutes} min", if n > 1 { "s" } else { "" })
        } else {
            format!(
                "{n} action{} · {minutes} min",
                if n == 1 { "" } else { "s" }
            )
        }
    }
    pub fn asks_permission(&self) -> &'static str {
        self.pick("demande une permission", "asks for permission")
    }
    pub fn allow(&self) -> &'static str {
        self.pick("Autoriser", "Allow")
    }
    pub fn deny(&self) -> &'static str {
        self.pick("Refuser", "Deny")
    }
    pub fn allowed(&self) -> &'static str {
        self.pick("Autorisé", "Allowed")
    }
    pub fn denied(&self) -> &'static str {
        self.pick("Refusé", "Denied")
    }
    pub fn by(&self) -> &'static str {
        self.pick("par", "by")
    }
    pub fn asks(&self) -> &'static str {
        self.pick("demande", "asks")
    }
    pub fn choose(&self) -> &'static str {
        self.pick("Choisir", "Choose")
    }
    pub fn other_answer(&self) -> &'static str {
        self.pick("Autre réponse…", "Other answer…")
    }
    pub fn answer_questions(&self, n: usize) -> String {
        if self.fr {
            format!("Répondre aux {n} questions…")
        } else {
            format!("Answer the {n} questions…")
        }
    }
    pub fn answered(&self) -> &'static str {
        self.pick("Répondu", "Answered")
    }
    pub fn your_answer(&self) -> &'static str {
        self.pick("Votre réponse", "Your answer")
    }
    pub fn other_or_details(&self) -> &'static str {
        self.pick("Autre réponse ou précisions", "Other answer or details")
    }
    pub fn proposes_plan(&self) -> &'static str {
        self.pick("propose un plan", "proposes a plan")
    }
    pub fn approve_plan(&self) -> &'static str {
        self.pick("Approuver le plan", "Approve the plan")
    }
    pub fn keep_planning(&self) -> &'static str {
        self.pick("Continuer à planifier…", "Keep planning…")
    }
    pub fn what_to_change(&self) -> &'static str {
        self.pick("Que changer ?", "What to change?")
    }
    pub fn plan_approved(&self) -> &'static str {
        self.pick("Plan approuvé", "Plan approved")
    }
    pub fn plan_sent_back(&self) -> &'static str {
        self.pick("Renvoyé au plan", "Sent back to planning")
    }
    pub fn whole_plan(&self) -> &'static str {
        self.pick(
            "Le plan entier est en pièce jointe.",
            "The whole plan is attached.",
        )
    }
    pub fn answered_elsewhere(&self) -> &'static str {
        self.pick("Répondu ailleurs", "Answered elsewhere")
    }
    pub fn started(&self) -> &'static str {
        self.pick("a démarré", "started")
    }
    pub fn stopped(&self) -> &'static str {
        self.pick("s'est arrêté", "stopped")
    }
    pub fn open_thread(&self) -> &'static str {
        self.pick("Ouvrir le fil", "Open the thread")
    }
    pub fn see_thread(&self) -> &'static str {
        self.pick("Voir le fil", "See the thread")
    }
    pub fn thread_intro(&self) -> &'static str {
        self.pick(
            "Fil ouvert : le travail de ce worker depuis son début, puis en direct. Ce que vous écrivez ici lui est envoyé.",
            "Thread opened: this worker's work since it started, then live. What you write here goes to it.",
        )
    }
    pub fn thread_ended(&self) -> &'static str {
        self.pick(
            "Le travail de ce worker est fini : le fil est archivé.",
            "This worker's work is over: the thread is archived.",
        )
    }
    pub fn delivery_ready(&self) -> &'static str {
        self.pick("Livraison prête", "Delivery ready")
    }
    pub fn land(&self) -> &'static str {
        self.pick("Fusionner", "Land")
    }
    pub fn send_back(&self) -> &'static str {
        self.pick("Renvoyer…", "Send back…")
    }
    pub fn what_to_fix(&self) -> &'static str {
        self.pick("Que corriger ?", "What to fix?")
    }
    pub fn landed(&self) -> &'static str {
        self.pick("Fusionné", "Landed")
    }
    pub fn landing(&self) -> &'static str {
        self.pick("Fusion en cours…", "Landing…")
    }
    pub fn not_landed(&self) -> &'static str {
        self.pick("Pas fusionné", "Not landed")
    }
    pub fn sent_back(&self) -> &'static str {
        self.pick("Renvoyé", "Sent back")
    }
    pub fn check(&self) -> &'static str {
        self.pick("Vérification", "Check")
    }
    pub fn review(&self) -> &'static str {
        self.pick("Relecture", "Review")
    }
    pub fn more_files(&self) -> &'static str {
        self.pick("et d'autres fichiers", "and more files")
    }
    pub fn dashboard(&self) -> &'static str {
        self.pick("Tableau de bord", "Dashboard")
    }
    pub fn idle(&self) -> &'static str {
        self.pick("inactif", "idle")
    }
    pub fn waiting(&self) -> &'static str {
        self.pick("vous attend", "waiting for you")
    }
    pub fn done(&self) -> &'static str {
        self.pick("a fini", "done")
    }
    pub fn starting(&self) -> &'static str {
        self.pick("démarre", "starting")
    }
    pub fn only_owner(&self) -> &'static str {
        self.pick(
            "Seul la personne qui a appairé Hangar peut répondre ici.",
            "Only the person who paired Hangar can answer here.",
        )
    }
    pub fn paired(&self) -> &'static str {
        self.pick(
            "Appairé : Hangar n'écoute plus que vous sur ce serveur.",
            "Paired: Hangar listens to you alone on this server now.",
        )
    }
    pub fn not_paired(&self) -> &'static str {
        self.pick(
            "Hangar n'est pas appairé : envoyez `pair <code>` avec le code que donne `hangar-node discord status` sur le serveur.",
            "Hangar is not paired: send `pair <code>` with the code `hangar-node discord status` gives on the server.",
        )
    }
    pub fn wrong_code(&self) -> &'static str {
        self.pick(
            "Ce n'est pas le code d'appairage.",
            "That is not the pairing code.",
        )
    }
    pub fn pairing_closed(&self) -> &'static str {
        self.pick(
            "Trop de codes faux : l'appairage est fermé. Relancez la connexion Discord de Hangar pour appairer.",
            "Too many wrong codes: pairing is closed. Restart Hangar's Discord connection to pair.",
        )
    }
    /// The agent `name` waits on an answer: what is written to it would be
    /// typed into its question.
    pub fn answer_first(&self, name: &str) -> String {
        if self.fr {
            format!(
                "« {name} » attend une réponse : répondez d'abord à sa carte, votre message serait tapé dans sa question."
            )
        } else {
            format!(
                "\"{name}\" waits on an answer: answer its card first, your message would be typed into its question."
            )
        }
    }
    pub fn no_longer_asked(&self) -> &'static str {
        self.pick("N'est plus demandé", "No longer asked")
    }
    pub fn no_orchestrator(&self) -> &'static str {
        self.pick(
            "Ce workspace n'a pas d'orchestrateur à qui parler.",
            "This workspace has no orchestrator to talk to.",
        )
    }
    pub fn workspace_made(&self) -> &'static str {
        self.pick("Workspace créé", "Workspace made")
    }
    pub fn command_stop(&self) -> &'static str {
        self.pick(
            "Arrêter le tour en cours de l'agent",
            "Stop the agent's turn",
        )
    }
    pub fn command_model(&self) -> &'static str {
        self.pick("Changer le modèle de l'agent", "Change the agent's model")
    }
    pub fn command_effort(&self) -> &'static str {
        self.pick("Changer l'effort de l'agent", "Change the agent's effort")
    }
    pub fn command_plan(&self) -> &'static str {
        self.pick("Passer l'agent en mode plan", "Put the agent in plan mode")
    }
    pub fn command_status(&self) -> &'static str {
        self.pick("Où en est ce workspace", "Where this workspace stands")
    }
    pub fn command_workspace(&self) -> &'static str {
        self.pick(
            "Créer un workspace dans un dossier du serveur",
            "Make a workspace in a folder of the server",
        )
    }
    pub fn sent(&self) -> &'static str {
        self.pick("Envoyé.", "Sent.")
    }
    pub fn nothing_working(&self) -> &'static str {
        self.pick("L'agent ne travaille pas.", "The agent is not working.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speaks_the_settings_language() {
        assert_eq!(Words::of(Some(Locale::Fr)).land(), "Fusionner");
        assert_eq!(Words::of(Some(Locale::En)).land(), "Land");
        assert_eq!(
            Words::of(Some(Locale::Fr)).actions(1, 2),
            "1 action · 2 min"
        );
        assert_eq!(
            Words::of(Some(Locale::En)).actions(3, 6),
            "3 actions · 6 min"
        );
    }
}
