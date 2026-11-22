/* ═══════════════════════════════════════════════════════════════
   HangarAI account: hangarai.dev/account

   Signs in with the HangarAI account (GitHub, through the account
   server) and shows what it holds: the license, its machines, the
   credits and their history; subscribes to Pro or Max, buys credits,
   opens Polar's customer portal for the subscription (Pro to Max
   and back are changed there) and the invoices; links the downloads.
   Signed in, it is an app: a sidebar (who, the tabs, the invoices,
   signing out) beside the tab's page.
   The tabs are the address's hash: the license and its credits;
   #referral, "Invite & earn", drawn by account/referral.js; #machines,
   the machines and the downloads. The server's side
   of it is docs/native/portal.md; ?mock=1 runs it against
   account/mock.js.

   Everything is drawn from `state` by render(), with DOM calls and
   textContent only: nothing the server sends becomes HTML.

     1. strings, English and French
     2. formats: numbers, prices, dates
     3. state, and the calls that change it
     4. the views
     5. start
   ═══════════════════════════════════════════════════════════════ */
(function () {
  'use strict';

  var Lang = window.HangarLang;
  var A = window.HangarAccount;
  var params = new URLSearchParams(location.search);
  var MOCK = params.get('mock') === '1';
  var PENDING = 'hangar.signin';
  var SIGN_IN_TTL = 10 * 60 * 1000;
  var reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  // Only whether this browser was signed in last time, so that the page
  // waits in the shape it is about to take: nothing of the account.
  var SEEN = 'hangar.account.seen';

  function seen(value) {
    // The site's header says "Account" rather than "Sign in" while it is set.
    if (value === true || value === false) document.documentElement.classList.toggle('signed', value);
    try {
      if (value === true) localStorage.setItem(SEEN, '1');
      else if (value === false) localStorage.removeItem(SEEN);
      else return localStorage.getItem(SEEN) === '1';
    } catch (e) { /* storage refused */ }
    return false;
  }

  /* ── 1. strings ────────────────────────────────────────────── */

  var STR = {
    en: {
      signin_title: 'Your HangarAI account',
      signin_lede: 'Sign in to see your license, your machines and your credits, and to manage your subscription and invoices.',
      signin_button: 'Sign in with GitHub',
      signin_fine: 'The account you sign in to from Settings › Hangar account in the app. GitHub tells us your name and email address, nothing else.',
      signin_privacy: 'Privacy',
      signin_new: 'No account yet? It is made the first time you sign in.',
      signin_key: 'Bought without an account? Sign in, then add your license key.',
      signing_in: 'Signing you in…',
      err_denied: 'The sign-in was cancelled at GitHub.',
      err_provider: 'GitHub did not complete the sign-in. Try again in a moment.',
      err_expired: 'That sign-in expired or was already used. Sign in again.',
      err_session: 'Your session ended. Sign in again.',
      err_signed_out: 'You are signed out.',
      err_rate: 'Too many attempts. Try again in a minute.',
      down_title: 'The account server can’t be reached',
      down_text: 'Check your connection, or try again in a moment. Your license keeps working in the app meanwhile.',
      retry: 'Try again',
      loading: 'Loading your account…',
      sign_out: 'Sign out',
      with_github: 'GitHub',

      license: 'License',
      licenses: 'Your licenses',
      kind_subscription: 'HangarAI Pro',
      kind_max: 'HangarAI Max',
      kind_lifetime: 'HangarAI Lifetime',
      kind_trial: 'Free trial',
      kind_free_grant: 'Free license',
      kind_unknown: 'License',
      plan_pro_monthly: 'Monthly',
      plan_pro_yearly: 'Yearly',
      plan_max_monthly: 'Monthly',
      plan_max_yearly: 'Yearly',
      st_active: 'Active',
      st_trial: 'Trial',
      st_free: 'Free',
      st_canceling: 'Won’t renew',
      st_past_due: 'Payment failed',
      st_ended: 'Ended',
      st_revoked: 'Revoked',
      renews: 'Renews on {date}',
      per_month: '{price} a month',
      per_year: '{price} a year',
      canceling_text: 'Active until {date}. Changed your mind? Renew it from Manage subscription.',
      past_due_text: 'The last payment didn’t go through. Polar tries again; update your card to keep HangarAI.',
      trial_left: '{n} days left · ends on {date}',
      trial_left_one: '1 day left · ends on {date}',
      trial_today: 'Ends today',
      trial_day: 'Day {n} of 14',
      trial_ended: 'Your trial ended on {date}.',
      ended_text: 'Ended on {date}. HangarAI is locked on its machines until you subscribe again; your purchased credits are kept.',
      lifetime_text: 'The app and every update, forever. Bought on {date}.',
      free_text: 'Thank you for using HangarAI before it was paid. This free license is your account’s, for one computer: the app and 30 credits a month. The statistics synced across your machines come with a paid license.',
      revoked_text: 'This license was revoked on {date}. If that’s a mistake, write to contact@hangarai.dev.',
      since_label: 'Since',
      license_id: 'License',
      manage: 'Manage subscription',
      update_card: 'Update payment',
      invoices: 'Invoices',
      offers_trial: 'Keep HangarAI after your trial',
      offers_free: 'Support HangarAI',
      offers_free_lede: 'If HangarAI helps you, a paid plan keeps it going, and adds the synced statistics and more credits. Your free license becomes that one.',
      offers_ended: 'Pick HangarAI up again',
      offers_none: 'Get HangarAI',
      offers_none_lede: 'No license on this account yet. Download HangarAI for a 14-day free trial, or buy it now. Bought one without an account? Add its key below.',
      offer_pro_monthly: 'Pro, monthly',
      offer_pro_yearly: 'Pro, yearly',
      offer_max_monthly: 'Max, monthly',
      offer_max_yearly: 'Max, yearly',
      offer_save: 'Save {price}',
      offer_credits: '{n} credits a month',
      offer_month: '/ month',
      offer_year: '/ year',
      upgrade_title: 'Need more? Move up to Max',
      upgrade_lede: '{n} credits a month instead of {m}, and your Cloud Builds first in the queue on {k} machines at once. {month} a month or {year} a year: Polar prorates the change, and the new credits start with your next period.',
      upgrade: 'Upgrade to Max',
      offers_fine: 'Prices include VAT. Payment and invoices by Polar, our merchant of record.',
      dl_get: 'Download {ext}',

      credits: 'Credits',
      // What one credit buys, from the server's rate: 1 credit ≈ 1.5 min of dictation.
      per_credit: '1 credit ≈ {n} {unit} of {feature}',
      // A rate of a credit or more a unit: Cloud build: 7.4 credits per min.
      per_unit: '{feature}: {n} credits per {unit}',
      equiv_hours: 'About {n} hours of {feature}',
      equiv_minutes: 'About {n} minutes of {feature}',
      credits_left: 'credits left',
      allowance: 'Monthly allowance',
      allowance_trial: 'Trial credits',
      allowance_left: '{left} of {total} left',
      resets: 'Resets on {date}. What is left doesn’t carry over.',
      trial_until: 'Until the trial ends, on {date}.',
      purchased: 'Purchased',
      purchased_left: '{n} left',
      purchased_fine: 'They never expire, and are spent once the allowance is gone.',
      purchased_ended: 'Kept on your license for when you subscribe again.',
      spent_period: 'This period',
      spent_none: 'Nothing spent yet this period.',
      buy_pack: 'Buy {n}',
      packs_fine: 'Packs never expire.',
      payg_title: 'Pay as you go',
      payg_lede: 'When your credits run out, hosted features keep going at {price} per {n} credits, up to the monthly limit you set.',
      payg_limit: 'Monthly limit',
      payg_other: 'Other',
      payg_custom: 'Limit in euros, {min} to {max}',
      payg_set: 'Set',
      payg_used: '{spent} of {limit} used this month',
      payg_credits: '{n} credits',
      payg_resets: 'The count starts again on {date}.',
      payg_off_note: 'Off: hosted features stop when your credits run out.',
      payg_checkout_note: 'Turning it on saves your card with Polar, on a free subscription that carries what you use.',
      payg_r_first_purchase: 'Available after your first purchase.',
      payg_r_invoice_unpaid: 'Paused: an invoice is unpaid. Pay it from Manage subscription to turn it back on.',
      payg_r_ended: 'Comes back with your subscription.',
      payg_on_toast: 'Pay as you go is on, up to {limit} a month.',
      payg_off_toast: 'Pay as you go is off.',
      payg_limit_toast: 'Monthly limit: {limit}.',
      err_payg_unavailable: 'Pay as you go isn’t available on this license.',
      err_bad_request: 'That value isn’t one we can take. Check it and try again.',
      h_payg: '{feature} · pay as you go',

      machines: 'Machines',
      machines_used: '{n} of {seats} in use',
      machines_active: 'Active machines',
      machines_manage: 'Manage machines and downloads',
      machine_unnamed: 'Machine {hash}',
      seen: 'seen {when}',
      activated: 'activated {date}',
      deactivate: 'Deactivate',
      deactivate_label: 'Deactivate {name}',
      machines_empty: 'No machine uses this license yet. Open HangarAI and enter your key, or sign in from Settings › License.',
      machines_fine: 'To move HangarAI to another computer, deactivate one here, then enter your key there or sign in from Settings › License.',
      confirm_title: 'Deactivate {name}?',
      confirm_text: 'HangarAI on it stops within a day and asks for a license. Its projects and settings stay on it, and you can activate it again later.',
      cancel: 'Cancel',
      deactivated: '{name} is deactivated.',

      history: 'Credit history',
      history_caption: 'Credits granted and spent, newest first',
      col_date: 'Date',
      col_what: 'What',
      col_credits: 'Credits',
      h_allowance: 'Monthly allowance',
      h_trial: 'Trial credits',
      h_lifetime: 'Included with Lifetime',
      h_purchase: 'Credit pack · {n}',
      h_purchase_any: 'Credit pack',
      h_gift: 'Gift',
      h_refund: 'Refund',
      h_admin: 'Adjustment',
      history_empty: 'Nothing yet. What you are granted and what hosted features spend shows here.',
      history_more: 'Show more',
      history_error: 'The history couldn’t be loaded.',

      seat_free_one: 'A free seat',
      seat_free: '{n} free seats',
      seat_free_text: 'On another computer, open HangarAI and enter your key, or sign in from Settings › License.',

      nav_license: 'License',
      nav_referral: 'Invite',
      nav_machines: 'Machines',
      nav_billing_short: 'Billing',
      nav_billing: 'Billing & invoices',
      nav_billing_where: '(at Polar)',
      page_license: 'Your license, your credits and their history.',
      page_referral: '30\u00a0% of what each friend pays, for as long as they pay.',
      page_machines: 'The computers your license is active on, and HangarAI for the next one.',

      downloads: 'Download HangarAI',
      version: 'Version {v}',
      this_computer: 'This computer',
      all_files: 'Every file of the latest release',
      downloads_fallback: 'The downloads are on the home page.',

      key_title: 'Have a license key?',
      key_label: 'License key',
      key_add: 'Add to my account',
      key_added: 'The license is on your account.',
      err_invalid_license: 'That isn’t a license key. Check it in the email you got after your purchase.',
      err_license_held: 'That license is on another HangarAI account.',

      feature_dictation: 'Dictation',
      unit_minute: 'min',

      thanks: 'Thank you! Your purchase shows here within a minute.',
      err_already_subscribed: 'This license already has that. Manage it from Manage subscription.',
      err_license_needed: 'Credits go on a license, and this account has none yet.',
      err_billing_unavailable: 'Payments are unavailable right now. Try again in a few minutes.',
      err_no_customer: 'Nothing bought on this account yet: invoices appear after your first purchase.',
      err_generic: 'That didn’t work. Try again in a moment.',
      err_network: 'The account server can’t be reached. Check your connection.',

      mock_label: 'Mock mode',
      mock_text: 'Made-up data, and nothing leaves this page.',
      mock_account: 'Account',
      mock_go: 'Mock mode: here HangarAI would open {url}',
      ok: 'OK',
      today: 'today',
      just_now: 'just now'
    },
    fr: {
      signin_title: 'Ton compte HangarAI',
      signin_lede: 'Connecte-toi pour voir ta licence, tes machines et tes crédits, et gérer ton abonnement et tes factures.',
      signin_button: 'Se connecter avec GitHub',
      signin_fine: 'Le compte auquel tu te connectes depuis Réglages › Compte Hangar dans l’app. GitHub nous donne ton nom et ton adresse e-mail, rien d’autre.',
      signin_privacy: 'Confidentialité',
      signin_new: 'Pas encore de compte\u00a0? Il est créé à ta première connexion.',
      signin_key: 'Acheté sans compte\u00a0? Connecte-toi, puis ajoute ta clé de licence.',
      signing_in: 'Connexion…',
      err_denied: 'La connexion a été annulée sur GitHub.',
      err_provider: 'GitHub n’a pas terminé la connexion. Réessaie dans un instant.',
      err_expired: 'Cette connexion a expiré ou a déjà servi. Connecte-toi à nouveau.',
      err_session: 'Ta session a pris fin. Connecte-toi à nouveau.',
      err_signed_out: 'Tu es déconnecté.',
      err_rate: 'Trop de tentatives. Réessaie dans une minute.',
      down_title: 'Le serveur de comptes est injoignable',
      down_text: 'Vérifie ta connexion, ou réessaie dans un instant. Ta licence continue de fonctionner dans l’app en attendant.',
      retry: 'Réessayer',
      loading: 'Chargement de ton compte…',
      sign_out: 'Se déconnecter',
      with_github: 'GitHub',

      license: 'Licence',
      licenses: 'Tes licences',
      kind_subscription: 'HangarAI Pro',
      kind_max: 'HangarAI Max',
      kind_lifetime: 'HangarAI Lifetime',
      kind_trial: 'Essai gratuit',
      kind_free_grant: 'Licence offerte',
      kind_unknown: 'Licence',
      plan_pro_monthly: 'Mensuel',
      plan_pro_yearly: 'Annuel',
      plan_max_monthly: 'Mensuel',
      plan_max_yearly: 'Annuel',
      st_active: 'Active',
      st_trial: 'Essai',
      st_free: 'Offerte',
      st_canceling: 'Ne sera pas renouvelé',
      st_past_due: 'Paiement refusé',
      st_ended: 'Terminée',
      st_revoked: 'Révoquée',
      renews: 'Renouvelé le {date}',
      per_month: '{price} par mois',
      per_year: '{price} par an',
      canceling_text: 'Active jusqu’au {date}. Tu changes d’avis\u00a0? Renouvelle-le depuis Gérer l’abonnement.',
      past_due_text: 'Le dernier paiement n’est pas passé. Polar réessaie\u00a0; mets ta carte à jour pour garder HangarAI.',
      trial_left: 'Encore {n} jours · se termine le {date}',
      trial_left_one: 'Encore 1 jour · se termine le {date}',
      trial_today: 'Se termine aujourd’hui',
      trial_day: 'Jour {n} sur 14',
      trial_ended: 'Ton essai s’est terminé le {date}.',
      ended_text: 'Terminée le {date}. HangarAI est verrouillé sur ses machines jusqu’à un nouvel abonnement\u00a0; tes crédits achetés sont conservés.',
      lifetime_text: 'L’app et toutes ses mises à jour, pour toujours. Achetée le {date}.',
      free_text: 'Merci d’avoir utilisé HangarAI avant qu’il soit payant. Cette licence offerte est celle de ton compte, pour un ordinateur\u00a0: l’app et 30 crédits par mois. Les statistiques synchronisées entre tes machines viennent avec une licence payante.',
      revoked_text: 'Cette licence a été révoquée le {date}. Si c’est une erreur, écris à contact@hangarai.dev.',
      since_label: 'Depuis le',
      license_id: 'Licence',
      manage: 'Gérer l’abonnement',
      update_card: 'Mettre à jour le paiement',
      invoices: 'Factures',
      offers_trial: 'Garder HangarAI après l’essai',
      offers_free: 'Soutenir HangarAI',
      offers_free_lede: 'Si HangarAI t’aide, une offre payante le fait vivre, et ajoute les statistiques synchronisées et plus de crédits. Ta licence offerte devient celle-là.',
      offers_ended: 'Reprendre HangarAI',
      offers_none: 'Obtenir HangarAI',
      offers_none_lede: 'Aucune licence sur ce compte pour l’instant. Télécharge HangarAI pour 14 jours d’essai gratuit, ou achète-le maintenant. Acheté sans compte\u00a0? Ajoute sa clé plus bas.',
      offer_pro_monthly: 'Pro, mensuel',
      offer_pro_yearly: 'Pro, annuel',
      offer_max_monthly: 'Max, mensuel',
      offer_max_yearly: 'Max, annuel',
      offer_save: '{price} d’économie',
      offer_credits: '{n} crédits par mois',
      offer_month: '/ mois',
      offer_year: '/ an',
      upgrade_title: 'Besoin de plus\u00a0? Passe à Max',
      upgrade_lede: '{n} crédits par mois au lieu de {m}, et tes Cloud Builds en tête de file sur {k} machines à la fois. {month} par mois ou {year} par an\u00a0: Polar calcule le changement au prorata, et les nouveaux crédits arrivent avec ta période suivante.',
      upgrade: 'Passer à Max',
      offers_fine: 'Prix TTC. Paiement et factures par Polar, notre revendeur officiel.',
      dl_get: 'Télécharger {ext}',

      credits: 'Crédits',
      per_credit: '1 crédit ≈ {n}\u00a0{unit} de {feature}',
      per_unit: '{feature}\u00a0: {n} crédits par {unit}',
      equiv_hours: 'Environ {n}\u00a0heures de {feature}',
      equiv_minutes: 'Environ {n}\u00a0minutes de {feature}',
      credits_left: 'crédits restants',
      allowance: 'Allocation du mois',
      allowance_trial: 'Crédits de l’essai',
      allowance_left: '{left} sur {total} restants',
      resets: 'Remise à zéro le {date}. Le reste n’est pas reporté.',
      trial_until: 'Jusqu’à la fin de l’essai, le {date}.',
      purchased: 'Achetés',
      purchased_left: '{n} restants',
      purchased_fine: 'Ils n’expirent jamais, et ne sont dépensés qu’une fois l’allocation épuisée.',
      purchased_ended: 'Gardés sur ta licence pour ton prochain abonnement.',
      spent_period: 'Cette période',
      spent_none: 'Rien de dépensé pour l’instant sur cette période.',
      buy_pack: 'Acheter {n}',
      packs_fine: 'Les recharges n’expirent jamais.',
      payg_title: 'Paiement à l’usage',
      payg_lede: 'Quand tes crédits sont épuisés, les fonctions hébergées continuent à {price} les {n} crédits, jusqu’à la limite mensuelle que tu fixes.',
      payg_limit: 'Limite mensuelle',
      payg_other: 'Autre',
      payg_custom: 'Limite en euros, de {min} à {max}',
      payg_set: 'Valider',
      payg_used: '{spent} sur {limit} utilisés ce mois-ci',
      payg_credits: '{n} crédits',
      payg_resets: 'Le compteur repart le {date}.',
      payg_off_note: 'Désactivé\u00a0: les fonctions hébergées s’arrêtent quand tes crédits sont épuisés.',
      payg_checkout_note: 'L’activer enregistre ta carte chez Polar, sur un abonnement gratuit qui porte ce que tu utilises.',
      payg_r_first_purchase: 'Disponible après ton premier achat.',
      payg_r_invoice_unpaid: 'En pause\u00a0: une facture est impayée. Règle-la depuis Gérer l’abonnement pour le réactiver.',
      payg_r_ended: 'Revient avec ton abonnement.',
      payg_on_toast: 'Le paiement à l’usage est activé, jusqu’à {limit} par mois.',
      payg_off_toast: 'Le paiement à l’usage est désactivé.',
      payg_limit_toast: 'Limite mensuelle\u00a0: {limit}.',
      err_payg_unavailable: 'Le paiement à l’usage n’est pas disponible sur cette licence.',
      err_bad_request: 'Cette valeur n’est pas acceptée. Vérifie-la et réessaie.',
      h_payg: '{feature} · à l’usage',

      machines: 'Machines',
      machines_used: '{n} sur {seats} utilisées',
      machines_active: 'Machines actives',
      machines_manage: 'Gérer les machines et les téléchargements',
      machine_unnamed: 'Machine {hash}',
      seen: 'vue {when}',
      activated: 'activée le {date}',
      deactivate: 'Désactiver',
      deactivate_label: 'Désactiver {name}',
      machines_empty: 'Aucune machine n’utilise encore cette licence. Ouvre HangarAI et saisis ta clé, ou connecte-toi depuis Réglages › Licence.',
      machines_fine: 'Pour passer HangarAI sur un autre ordinateur, désactive l’un des tiens ici, puis saisis ta clé sur le nouveau ou connecte-toi depuis Réglages › Licence.',
      confirm_title: 'Désactiver {name}\u00a0?',
      confirm_text: 'HangarAI s’y arrête dans la journée et demande une licence. Ses projets et réglages y restent, et tu pourras la réactiver plus tard.',
      cancel: 'Annuler',
      deactivated: '{name} est désactivée.',

      history: 'Historique des crédits',
      history_caption: 'Crédits reçus et dépensés, du plus récent au plus ancien',
      col_date: 'Date',
      col_what: 'Quoi',
      col_credits: 'Crédits',
      h_allowance: 'Allocation du mois',
      h_trial: 'Crédits de l’essai',
      h_lifetime: 'Inclus avec Lifetime',
      h_purchase: 'Recharge · {n}',
      h_purchase_any: 'Recharge',
      h_gift: 'Cadeau',
      h_refund: 'Remboursement',
      h_admin: 'Ajustement',
      history_empty: 'Rien pour l’instant. Ce que tu reçois et ce que dépensent les fonctions hébergées s’affiche ici.',
      history_more: 'Afficher plus',
      history_error: 'L’historique n’a pas pu être chargé.',

      seat_free_one: 'Une place libre',
      seat_free: '{n} places libres',
      seat_free_text: 'Sur un autre ordinateur, ouvre HangarAI et saisis ta clé, ou connecte-toi depuis Réglages › Licence.',

      nav_license: 'Licence',
      nav_referral: 'Parrainage',
      nav_machines: 'Machines',
      nav_billing_short: 'Factures',
      nav_billing: 'Factures et paiement',
      nav_billing_where: '(chez Polar)',
      page_license: 'Ta licence, tes crédits et leur historique.',
      page_referral: '30 % de ce que paie chaque filleul, tant qu’il paie.',
      page_machines: 'Les ordinateurs où ta licence est active, et HangarAI pour le suivant.',

      downloads: 'Télécharger HangarAI',
      version: 'Version {v}',
      this_computer: 'Cet ordinateur',
      all_files: 'Tous les fichiers de la dernière version',
      downloads_fallback: 'Les téléchargements sont sur la page d’accueil.',

      key_title: 'Tu as une clé de licence\u00a0?',
      key_label: 'Clé de licence',
      key_add: 'Ajouter à mon compte',
      key_added: 'La licence est sur ton compte.',
      err_invalid_license: 'Ce n’est pas une clé de licence. Vérifie-la dans l’e-mail reçu après ton achat.',
      err_license_held: 'Cette licence est sur un autre compte HangarAI.',

      feature_dictation: 'Dictée',
      unit_minute: 'min',

      thanks: 'Merci\u00a0! Ton achat s’affiche ici d’ici une minute.',
      err_already_subscribed: 'Cette licence l’a déjà. Gère-la depuis Gérer l’abonnement.',
      err_license_needed: 'Les crédits vont sur une licence, et ce compte n’en a pas encore.',
      err_billing_unavailable: 'Les paiements sont indisponibles pour le moment. Réessaie dans quelques minutes.',
      err_no_customer: 'Rien d’acheté sur ce compte pour l’instant\u00a0: les factures apparaissent après ton premier achat.',
      err_generic: 'Ça n’a pas marché. Réessaie dans un instant.',
      err_network: 'Le serveur de comptes est injoignable. Vérifie ta connexion.',

      mock_label: 'Mode démo',
      mock_text: 'Données inventées, et rien ne quitte cette page.',
      mock_account: 'Compte',
      mock_go: 'Mode démo\u00a0: ici HangarAI ouvrirait {url}',
      ok: 'OK',
      today: 'aujourd’hui',
      just_now: 'à l’instant'
    }
  };

  // "Invite & earn" brings its own strings (account/referral.js).
  var RF_MOD = window.HangarReferral;
  if (RF_MOD) {
    Object.assign(STR.en, RF_MOD.STR.en);
    Object.assign(STR.fr, RF_MOD.STR.fr);
  }

  function lang() {
    return Lang ? Lang.get() : 'en';
  }

  function t(key, vars) {
    var text = STR[lang()][key];
    if (text === undefined) text = STR.en[key] !== undefined ? STR.en[key] : key;
    if (vars) {
      text = text.replace(/\{(\w+)\}/g, function (m, name) {
        return vars[name] !== undefined ? vars[name] : m;
      });
    }
    return text;
  }

  /* ── 2. formats ────────────────────────────────────────────── */

  // French groups digits with a narrow no-break space (U+202F) that
  // the site's Inter subset has no glyph for: "1000" on screen. The
  // page's own copy writes a no-break space; numbers here do the same.
  function nbsp(text) {
    return text.replace(/\u202f/g, '\u00a0');
  }

  // Credits are counted in fractions (milli-credits on the server); one
  // decimal at most is shown.
  function num(n) {
    return nbsp(new Intl.NumberFormat(lang(), { maximumFractionDigits: 1 }).format(n));
  }

  function price(p) {
    if (!p) return '';
    var whole = p.amount % 100 === 0;
    return nbsp(new Intl.NumberFormat(lang(), {
      style: 'currency', currency: p.currency || 'EUR',
      minimumFractionDigits: whole ? 0 : 2, maximumFractionDigits: whole ? 0 : 2
    }).format(p.amount / 100));
  }

  // In French the first of a month is "1er" (le 1er décembre 2026).
  function date(ms) {
    var d = new Date(ms);
    var text = new Intl.DateTimeFormat(lang(), { day: 'numeric', month: 'long', year: 'numeric' }).format(d);
    return lang() === 'fr' && d.getDate() === 1 ? text.replace(/^1(?=\s)/, '1er') : text;
  }

  function shortDate(ms) {
    var d = new Date(ms);
    var sameYear = d.getFullYear() === new Date().getFullYear();
    return new Intl.DateTimeFormat(lang(), sameYear ? { day: 'numeric', month: 'short' } : { day: 'numeric', month: 'short', year: 'numeric' }).format(d);
  }

  // "2 hours ago", "yesterday", "3 days ago", then a date.
  function ago(ms) {
    var diff = Date.now() - ms;
    if (diff < 60000) return t('just_now');
    var rtf = new Intl.RelativeTimeFormat(lang(), { numeric: 'auto' });
    if (diff < 3600000) return rtf.format(-Math.round(diff / 60000), 'minute');
    if (diff < 86400000) return rtf.format(-Math.round(diff / 3600000), 'hour');
    if (diff < 14 * 86400000) return rtf.format(-Math.round(diff / 86400000), 'day');
    return shortDate(ms);
  }

  // "About" figures: whole hours, and from 20 on, down to a multiple of 5
  // (as assets/offers.js does on the pricing pages).
  function approxHours(h) {
    return h >= 20 ? Math.floor(h / 5) * 5 : Math.max(1, Math.round(h));
  }

  function daysUntil(ms) {
    return Math.max(0, Math.ceil((ms - Date.now()) / 86400000));
  }

  function featureName(id) {
    var key = 'feature_' + id;
    return STR.en[key] ? t(key) : id;
  }

  function unitName(unit) {
    var key = 'unit_' + unit;
    return STR.en[key] ? t(key) : unit;
  }

  function osName(os) {
    return { windows: 'Windows', macos: 'macOS', linux: 'Linux' }[os] || os || '';
  }

  /* ── 3. state ──────────────────────────────────────────────── */

  var api = null;
  var TABS = ['license', 'referral', 'machines'];

  // The tab is the address's hash: #referral is "Invite & earn" (the
  // app's Settings › License links there), #machines the machines and
  // the downloads, anything else the license and its credits.
  function tabOf(hash) {
    if (RF_MOD && /^#referral$/.test(hash || '')) return 'referral';
    return /^#machines$/.test(hash || '') ? 'machines' : 'license';
  }

  var state = {
    tab: tabOf(location.hash),
    view: 'loading',      // loading | signedout | signingin | down | ready
    notice: null,         // { tone, key } over the sign-in card
    summary: null,
    current: null,        // the license on screen
    history: {},          // license id → { entries, next, loading, error }
    downloads: null,      // { version, plats, all } once read from the home page
    busy: {},             // what is under way: 'buy:<product>', 'portal', 'seat:<id>', 'key', 'signin'
    keyError: null,
    flash: null           // an error shown in the license panel
  };
  var after = null;       // a selector to focus once drawn

  function license() {
    var list = (state.summary && state.summary.licenses) || [];
    for (var i = 0; i < list.length; i++) if (list[i].id === state.current) return list[i];
    return list[0] || null;
  }

  function errorKey(err) {
    var code = err && err.code;
    if (code === 'network') return 'err_network';
    if (code === 'rate_limited') return 'err_rate';
    if (STR.en['err_' + code]) return 'err_' + code;
    return 'err_generic';
  }

  function readPending() {
    try {
      var raw = sessionStorage.getItem(PENDING);
      sessionStorage.removeItem(PENDING);
      return raw ? JSON.parse(raw) : null;
    } catch (e) {
      return null;
    }
  }

  // The address bar without what a sign-in or a checkout came back with.
  function cleanUrl(keys) {
    var url = new URL(location.href);
    keys.forEach(function (k) { url.searchParams.delete(k); });
    history.replaceState(null, '', url.pathname + (url.search ? url.search : '') + url.hash);
  }

  function signIn() {
    state.busy.signin = true;
    render();
    var next = params.get('buy');
    A.pkce().then(function (p) {
      try {
        sessionStorage.setItem(PENDING, JSON.stringify({
          state: p.state, verifier: p.verifier, at: Date.now(),
          buy: A.PRODUCTS.indexOf(next) >= 0 ? next : null,
          tab: state.tab
        }));
      } catch (e) { /* without storage the return is refused as expired, and says so */ }
      location.assign(api.startUrl(p.state, p.challenge));
    }, function () {
      state.busy.signin = false;
      state.notice = { tone: 'error', key: 'err_generic' };
      render();
    });
  }

  function finishSignIn(code, returnedState, error) {
    var pending = readPending();
    cleanUrl(['code', 'state', 'error']);
    // GitHub's way back drops the hash: the tab comes from the sign-in.
    if (pending && TABS.indexOf(pending.tab) > 0) {
      state.tab = pending.tab;
      history.replaceState(null, '', location.pathname + location.search + '#' + pending.tab);
    }
    if (error) {
      state.view = 'signedout';
      state.notice = { tone: error === 'access_denied' ? 'warn' : 'error', key: error === 'access_denied' ? 'err_denied' : 'err_provider' };
      return render();
    }
    if (!pending || pending.state !== returnedState || Date.now() - pending.at > SIGN_IN_TTL) {
      state.view = 'signedout';
      state.notice = { tone: 'warn', key: 'err_expired' };
      return render();
    }
    state.view = 'signingin';
    render();
    api.session(code, pending.verifier).then(function () {
      if (pending.buy) params.set('buy', pending.buy);
      load({ focus: '#acct-title' });
    }, function (err) {
      state.view = err.code === 'network' ? 'down' : 'signedout';
      state.notice = { tone: 'error', key: err.code === 'invalid_code' ? 'err_expired' : errorKey(err) };
      render();
    });
  }

  function load(opts) {
    opts = opts || {};
    if (!state.summary) {
      state.view = 'loading';
      render();
    }
    return api.summary().then(function (summary) {
      seen(true);
      state.summary = summary;
      var still = summary.licenses.some(function (l) { return l.id === state.current; });
      if (!still) state.current = summary.licenses.length ? summary.licenses[0].id : null;
      var lic = license();
      // The first page of the history before the page is drawn, the first
      // time: it comes in whole rather than growing under the reader.
      var first = state.view !== 'ready' && lic && !state.history[lic.id];
      var history = lic && (!state.history[lic.id] || opts.fresh)
        ? loadHistory(lic.id, true, first) : Promise.resolve();
      return (first ? history : Promise.resolve()).then(function () {
        state.view = 'ready';
        after = opts.focus || after;
        render();
        if (state.tab === 'referral' && RF) RF.load();
        resumeBuy();
      });
    }, function (err) {
      if (err.status === 401) {
        seen(false);
        var was = state.view === 'ready';
        state.summary = null;
        state.view = 'signedout';
        state.notice = was || opts.focus ? { tone: 'warn', key: 'err_session' } : state.notice;
      } else {
        state.view = 'down';
      }
      render();
    });
  }

  // The first page holds a dozen lines; each "Show more" thirty.
  function loadHistory(id, fresh, quiet) {
    var h = state.history[id];
    if (!h || fresh) h = state.history[id] = { entries: [], next: null, loading: false, error: false, done: false };
    if (h.loading) return Promise.resolve();
    h.loading = true;
    h.error = false;
    if (!quiet) render();
    var count = h.entries.length;
    return api.history(id, fresh ? null : h.next, count ? 30 : 12).then(function (page) {
      h.entries = h.entries.concat(page.entries || []);
      h.next = page.next || null;
      h.done = true;
      h.loading = false;
      if (!fresh && count) after = '#ledger-row-' + count;
      if (!quiet) render();
    }, function (err) {
      h.loading = false;
      h.error = true;
      if (err.status === 401 && !quiet) return load();
      if (!quiet) render();
    });
  }

  // Where a checkout or the portal sends the browser: Polar's own
  // pages, and in mock mode a dialog that says so instead.
  function go(url) {
    if (api.mock) {
      return dialog({ title: t('mock_label'), text: t('mock_go', { url: url }), confirm: t('ok'), cancel: null });
    }
    if (!A.isPolarUrl(url)) {
      state.flash = 'err_billing_unavailable';
      return render();
    }
    location.assign(url);
  }

  function buy(product, lic, extra) {
    var key = 'buy:' + product;
    if (state.busy[key]) return;
    state.busy[key] = true;
    state.flash = null;
    render();
    var body = Object.assign({ product: product, 'return': 'account' }, extra || {});
    if (lic) body.license = lic.id;
    api.checkout(body).then(function (res) {
      delete state.busy[key];
      render();
      go(res.url);
    }, function (err) {
      delete state.busy[key];
      if (err.status === 401) return load();
      state.flash = errorKey(err);
      after = '#flash';
      render();
    });
  }

  /* pay as you go ------------------------------------------------- */

  function payg(lic) {
    return lic && lic.credits && lic.credits.payg;
  }

  function setPayg(lic, body, toastKey) {
    if (state.busy.payg) return;
    state.busy.payg = true;
    state.paygError = null;
    render();
    api.setPayg(lic.id, body).then(function (res) {
      delete state.busy.payg;
      lic.credits.payg = res.payg;
      state.paygCustom = false;
      toast(t(toastKey, { limit: price(res.payg.limit) }));
      after = '#payg-switch';
      render();
    }, function (err) {
      delete state.busy.payg;
      if (err.status === 401) return load();
      if (err.code === 'payg_checkout_needed') return buy('payg', lic, { payg_limit: body.limit });
      state.paygError = errorKey(err);
      render();
    });
  }

  function togglePayg(lic) {
    var p = payg(lic);
    if (p.enabled) return setPayg(lic, { enabled: false }, 'payg_off_toast');
    var presets = (p.limits && p.limits.presets) || [];
    var limit = p.limit || { amount: presets[1] || presets[0] || 1000, currency: 'EUR' };
    // Without a subscription to carry it, the first time goes through
    // Polar to save a card; the webhook turns it on with this limit.
    if (p.setup === 'checkout') return buy('payg', lic, { payg_limit: limit });
    setPayg(lic, { enabled: true, limit: limit }, 'payg_on_toast');
  }

  function setLimit(lic, amount) {
    var p = payg(lic);
    setPayg(lic, { enabled: true, limit: { amount: amount, currency: (p.limit && p.limit.currency) || 'EUR' } }, 'payg_limit_toast');
  }

  // ?buy=<product>, from buy/ when a pack needed a license: once signed
  // in, the checkout for the license on screen.
  function resumeBuy() {
    var product = params.get('buy');
    if (!product) return;
    params.delete('buy');
    cleanUrl(['buy']);
    if (A.PRODUCTS.indexOf(product) < 0) return;
    var lic = license();
    if (/^credits_/.test(product) && !lic) {
      state.flash = 'err_license_needed';
      return render();
    }
    buy(product, lic);
  }

  // From the license panel, a refusal shows in it; from the sidebar,
  // which stays beside every tab, in a toast.
  function openPortal(opts) {
    var fromSide = !!(opts && opts.side);
    if (state.busy.portal) return;
    state.busy.portal = true;
    state.flash = null;
    render();
    var lic = license();
    api.billingPortal(lic ? { license: lic.id } : {}).then(function (res) {
      delete state.busy.portal;
      render();
      go(res.url);
    }, function (err) {
      delete state.busy.portal;
      if (err.status === 401) return load();
      if (fromSide) {
        toast(t(errorKey(err)));
        return render();
      }
      state.flash = errorKey(err);
      after = '#flash';
      render();
    });
  }

  function machineName(m) {
    return m.name || t('machine_unnamed', { hash: m.machine });
  }

  function deactivate(lic, m) {
    var name = machineName(m);
    dialog({
      title: t('confirm_title', { name: name }),
      text: t('confirm_text'),
      confirm: t('deactivate'),
      cancel: t('cancel'),
      danger: true
    }).then(function (yes) {
      if (!yes) return;
      var key = 'seat:' + m.id;
      state.busy[key] = true;
      render();
      api.deactivate(lic.id, m.id).then(function () {
        delete state.busy[key];
        toast(t('deactivated', { name: name }));
        load({ focus: '#machines-title' });
      }, function (err) {
        delete state.busy[key];
        if (err.status === 401) return load();
        toast(t(errorKey(err)));
        render();
      });
    });
  }

  function claim(key) {
    if (state.busy.key) return;
    state.busy.key = true;
    state.keyError = null;
    render();
    api.claim(key).then(function (res) {
      delete state.busy.key;
      state.current = res.license.id;
      toast(t('key_added'));
      load({ focus: '#lic-title', fresh: true });
    }, function (err) {
      delete state.busy.key;
      if (err.status === 401) return load();
      state.keyError = errorKey(err);
      after = '#key-input';
      render();
    });
  }

  function signOut() {
    state.busy.signout = true;
    render();
    api.signOut().then(done, done);
    function done() {
      seen(false);
      if (RF) RF.reset();
      state = Object.assign(state, { view: 'signedout', summary: null, history: {}, busy: {}, flash: null,
        notice: { tone: 'info', key: 'err_signed_out' } });
      after = '#signin-title';
      render();
    }
  }

  /* ── 4. the views ──────────────────────────────────────────── */

  // h('p.cls#id', {attrs}, children…): an element, its text by
  // textContent. Children may be strings, nodes, arrays, or nothing.
  function h(spec, attrs) {
    var m = /^([a-z0-9]+)((?:[.#][\w-]+)*)$/.exec(spec);
    var el = document.createElement(m[1]);
    (m[2].match(/[.#][\w-]+/g) || []).forEach(function (part) {
      if (part[0] === '.') el.classList.add(part.slice(1));
      else el.id = part.slice(1);
    });
    var kids = Array.prototype.slice.call(arguments, 2);
    if (attrs && (typeof attrs !== 'object' || attrs.nodeType || Array.isArray(attrs))) {
      kids.unshift(attrs);
      attrs = null;
    }
    if (attrs) {
      Object.keys(attrs).forEach(function (k) {
        var v = attrs[k];
        if (v === null || v === undefined || v === false) return;
        if (k.slice(0, 2) === 'on') el.addEventListener(k.slice(2), v);
        else if (k === 'style') el.setAttribute('style', v);
        else el.setAttribute(k, v === true ? '' : String(v));
      });
    }
    (function append(list) {
      list.forEach(function (kid) {
        if (kid === null || kid === undefined || kid === false) return;
        if (Array.isArray(kid)) return append(kid);
        el.appendChild(kid.nodeType ? kid : document.createTextNode(String(kid)));
      });
    })(kids);
    return el;
  }

  function svg(paths, size) {
    var ns = 'http://www.w3.org/2000/svg';
    var el = document.createElementNS(ns, 'svg');
    el.setAttribute('width', size || 16);
    el.setAttribute('height', size || 16);
    el.setAttribute('viewBox', '0 0 16 16');
    el.setAttribute('aria-hidden', 'true');
    el.innerHTML = paths; // constant markup of this file, never data
    return el;
  }

  var ICON = {
    github: '<path fill="currentColor" d="M8 0a8 8 0 0 0-2.53 15.59c.4.07.55-.17.55-.38l-.01-1.34c-2.23.48-2.7-1.07-2.7-1.07-.36-.93-.89-1.18-.89-1.18-.73-.5.05-.49.05-.49.8.06 1.23.83 1.23.83.72 1.23 1.88.87 2.34.67.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82a7.6 7.6 0 0 1 4 0c1.53-1.03 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.28.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48l-.01 2.2c0 .21.15.46.55.38A8 8 0 0 0 8 0Z"/>',
    computer: '<rect x="1.75" y="2.75" width="12.5" height="8.5" rx="1.25" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M5.5 13.75h5M8 11.25v2.5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>',
    offline: '<circle cx="8" cy="8" r="6.25" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M8 4.75v3.75" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><circle cx="8" cy="11" r=".85" fill="currentColor"/>',
    external: '<path d="M6.5 3.25h-3a1 1 0 0 0-1 1v8.25a1 1 0 0 0 1 1h8.25a1 1 0 0 0 1-1v-3M9.25 2.5h4.25v4.25M13.25 2.75 7.5 8.5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>',
    license: '<circle cx="5.25" cy="10.75" r="2.75" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="m7.25 8.75 5.5-5.5M10.5 5.5l1.75 1.75M12.25 3.75 13.5 5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>',
    gift: '<rect x="2.25" y="5.75" width="11.5" height="3" rx=".75" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M3.25 8.75v4.5c0 .41.34.75.75.75h8c.41 0 .75-.34.75-.75v-4.5M8 5.75V14M8 5.75S6.75 2.25 4.9 2.5c-1.2.16-1.25 1.9-.05 2.55C5.9 5.6 8 5.75 8 5.75Zm0 0s1.25-3.5 3.1-3.25c1.2.16 1.25 1.9.05 2.55C10.1 5.6 8 5.75 8 5.75Z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>',
    receipt: '<path d="M3.5 1.75h9v12.5l-1.5-1-1.5 1-1.5-1-1.5 1-1.5-1-1.5 1V1.75Z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/><path d="M5.75 5.25h4.5M5.75 8h4.5M5.75 10.75H8" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>',
    signout: '<path d="M6.25 2.75h-2.5a1 1 0 0 0-1 1v8.5a1 1 0 0 0 1 1h2.5M10.25 5l3 3-3 3M13 8H6.5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>',
    plus: '<path d="M8 3.25v9.5M3.25 8h9.5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>'
  };

  function brandMark(size) {
    var ns = 'http://www.w3.org/2000/svg';
    var el = document.createElementNS(ns, 'svg');
    el.setAttribute('width', size);
    el.setAttribute('height', size);
    el.setAttribute('viewBox', '200 200 624 624');
    el.setAttribute('aria-hidden', 'true');
    el.innerHTML = '<rect x="200" y="200" width="260" height="624" fill="var(--accent)"></rect>' +
      '<rect x="538" y="200" width="286" height="260" fill="var(--fg-hi)"></rect>' +
      '<rect x="538" y="538" width="286" height="286" fill="var(--fg-hi)"></rect>';
    return el;
  }

  function button(label, opts) {
    opts = opts || {};
    var cls = 'btn' + (opts.primary ? ' btn--primary' : '') + (opts.quiet ? ' btn--quiet' : '') +
      (opts.small ? ' btn--sm' : '') + (opts.block ? ' btn--block' : '') + (opts.cls ? ' ' + opts.cls : '');
    var el = h('button', {
      type: opts.submit ? 'submit' : 'button',
      'class': cls,
      'aria-busy': opts.busy ? 'true' : null,
      'aria-label': opts.label || null,
      id: opts.id || null,
      onclick: opts.busy ? null : opts.onclick
    }, opts.icon ? svg(ICON[opts.icon]) : null, label, opts.external ? svg(ICON.external, 14) : null);
    return el;
  }

  function pill(text, tone) {
    return h('span.pill.pill--' + tone, text);
  }

  /* the views of the page: one of them at a time ------------------ */

  function viewSignIn() {
    var busy = !!state.busy.signin || state.view === 'signingin';
    var notice = state.notice;
    var invite = state.tab === 'referral';
    return h('section.signin', { 'aria-labelledby': 'signin-title' },
      h('div.signin__card' + (invite ? '.signin__card--invite' : ''),
        h('div.signin__mark', brandMark(36)),
        h('h1.signin__title#signin-title', { tabindex: '-1' }, t(invite ? 'rf_signin_title' : 'signin_title')),
        h('p.signin__lede', t(invite ? 'rf_signin_lede' : 'signin_lede')),
        notice ? h('div.notice.notice--' + notice.tone, { role: notice.tone === 'error' ? 'alert' : 'status' }, h('p', t(notice.key))) : null,
        state.view === 'signingin'
          ? h('p.signin__wait', { role: 'status' }, h('span.spinner', { 'aria-hidden': 'true' }), t('signing_in'))
          : button(t('signin_button'), { primary: true, block: true, icon: 'github', busy: busy, cls: 'btn--lg', onclick: signIn }),
        h('p.signin__fine', t('signin_fine'), ' ', h('a', { href: '../privacy.html' }, t('signin_privacy')), '.')
      ),
      h('ul.signin__more',
        h('li', t('signin_new')),
        h('li', t('signin_key'))
      )
    );
  }

  function viewDown() {
    return h('section.signin', { 'aria-labelledby': 'down-title' },
      h('div.signin__card',
        h('div.signin__mark.signin__mark--down', svg(ICON.offline, 28)),
        h('h1.signin__title#down-title', { tabindex: '-1' }, t('down_title')),
        h('p.signin__lede', t('down_text')),
        button(t('retry'), { primary: true, block: true, busy: state.busy.retry, onclick: function () {
          state.busy.retry = true;
          render();
          load().then(function () { delete state.busy.retry; render(); });
        } })
      )
    );
  }

  // The sign-in card's frame, while the page learns whether it is needed.
  function viewWait() {
    return h('section.signin', { 'aria-busy': 'true' },
      h('div.signin__card.signin__card--wait',
        h('div.signin__mark', brandMark(36)),
        h('p.signin__wait', { role: 'status' }, h('span.spinner', { 'aria-hidden': 'true' }), t('loading'))
      )
    );
  }

  // The app's shape, drawn in lines while the account comes in: the
  // sidebar, the page's title and the panels of the tab asked for, so
  // that nothing moves when they arrive.
  function viewLoading() {
    function bone(cls) { return h('div.bone' + (cls ? '.' + cls : '')); }
    var body;
    if (state.tab === 'referral' && RF) {
      body = h('div.rf.rf--loading', { 'aria-hidden': 'true' },
        h('div.panel.panel--bones.rf-hero--bones', bone('bone--line'), bone('bone--hero'), bone('bone--line'), bone('bone--meter')),
        h('div.rf-row.rf-row--a',
          h('div.panel.panel--bones', bone('bone--line'), bone('bone--row'), bone('bone--row')),
          h('div.panel.panel--bones', bone('bone--line'), bone('bone--big'), bone('bone--line')),
          h('div.panel.panel--bones.rf-funnel', bone('bone--line'), bone('bone--row'), bone('bone--row'))));
    } else if (state.tab === 'machines') {
      body = h('div.acct-stack', { 'aria-hidden': 'true' },
        h('div.panel.panel--bones', bone('bone--line'), bone('bone--row'), bone('bone--row')),
        h('div.panel.panel--bones', bone('bone--line'), bone('bone--row')));
    } else {
      body = h('div.lic-grid', { 'aria-hidden': 'true' },
        h('div.lic-grid__a', h('div.panel.panel--bones.lic', bone('bone--line'), bone('bone--title'), bone('bone--line'), bone('bone--line'))),
        h('div.panel.panel--bones.credits', bone('bone--line'), bone('bone--big'), bone('bone--line'), bone('bone--line')),
        h('div.panel.panel--bones.history', bone('bone--line'), bone('bone--row'), bone('bone--row'), bone('bone--row')));
    }
    return h('div.acct-app.acct-app--loading', { 'aria-busy': 'true' },
      h('div.acct-side', { 'aria-hidden': 'true' },
        h('div.acct-side__in',
          h('div.acct-who', h('div.bone.bone--avatar'), h('div.bone-stack', bone('bone--name'), bone('bone--mail'))),
          h('div.acct-nav.acct-nav--bones', bone('bone--nav'), bone('bone--nav'), bone('bone--nav')))),
      h('div.acct-main',
        h('div.acct-page',
          h('p.sr-only', { role: 'status' }, t('loading')),
          h('div.acct-page__head', { 'aria-hidden': 'true' }, h('div.bone-stack', bone('bone--title'), bone('bone--line'))),
          body))
    );
  }

  // The app: the sidebar (who is signed in, the sections, the invoices,
  // signing out) and the tab's page beside it. On a phone the sidebar
  // becomes a row with the name and a tab bar that stays under the bar.
  function viewReady() {
    var s = state.summary;
    return h('div.acct-app',
      sidebar(s),
      h('div.acct-main', h('div.acct-page.acct-page--' + pageTab(), pageBody(s)))
    );
  }

  function pageTab() {
    return state.tab === 'referral' && !RF ? 'license' : state.tab;
  }

  function pageBody(s) {
    var tab = pageTab();
    var lic = license();
    var head = h('header.acct-page__head',
      h('h1.acct-page__title#acct-title', { tabindex: '-1' },
        t(tab === 'referral' ? 'tab_referral' : tab === 'machines' ? 'machines' : 'tab_license')),
      h('p.acct-page__lede', t('page_' + tab)));
    var several = tab !== 'referral' && s.licenses.length > 1 ? picker(s.licenses) : null;
    if (tab === 'referral') return [head, RF.view()];
    if (tab === 'machines') return [head, several, lic ? machinesPanel(lic) : null, downloadsPanel()];
    if (!lic) return [head, h('div.acct-stack', nonePanel(s), downloadsPanel(), keyPanel())];
    // Wide, the license, its credits and their history side by side;
    // narrower, the license over the two others; on a phone, one column.
    return [head, several,
      h('div.lic-grid',
        h('div.lic-grid__a', licensePanel(lic, s), seatsPanel(lic), keyPanel()),
        creditsPanel(lic, s),
        historyPanel(lic))];
  }

  // The machines at a glance, beside the license: the Machines tab has
  // the rest (deactivating one, the downloads).
  function seatsPanel(l) {
    return h('section.panel.seats-card', { 'aria-labelledby': 'seats-title' },
      h('div.panel__head',
        h('h2.panel__title#seats-title', t('machines')),
        h('p.panel__aside', t('machines_used', { n: num(l.machines.length), seats: num(l.seats) }))),
      h('div.seats', { 'aria-hidden': 'true' },
        Array.apply(null, Array(Math.min(l.seats, 12))).map(function (_, i) {
          return h('i' + (i < l.machines.length ? '.on' : ''));
        })),
      l.machines.length
        ? h('ul.seats-card__list', l.machines.map(function (m) {
          return h('li',
            h('span.seats-card__icon', svg(ICON.computer, 14)),
            h('span.seats-card__name', machineName(m)),
            h('span.seats-card__meta', [osName(m.os), t('seen', { when: ago(m.last_seen_at) })].filter(Boolean).join(' · ')));
        }))
        : h('p.machines__empty', t('machines_empty')),
      h('p.seats-card__more', h('a', { href: '#machines', onclick: function (ev) { goTab(ev, 'machines'); } },
        t('machines_manage'), ' ', h('span', { 'aria-hidden': 'true' }, '\u2192')))
    );
  }

  // A link to another tab of the page: the hash says which, and the
  // browser's back button goes between them.
  function goTab(ev, tab) {
    if (ev.metaKey || ev.ctrlKey || ev.shiftKey || ev.altKey || ev.button) return;
    ev.preventDefault();
    if (tab === state.tab) return;
    history.pushState(null, '', location.pathname + location.search + '#' + tab);
    showTab(tab);
  }

  function showTab(tab) {
    if (tab === state.tab) return;
    state.tab = tab;
    if (tab === 'referral' && RF && state.view === 'ready') RF.load({ quiet: true });
    render();
    // Another tab starts at its top, under the bar (and on a phone the
    // tab bar): never in the middle of a page that was not there.
    var main = document.querySelector('.acct-main');
    var bar = document.querySelector('.bar');
    var nav = document.querySelector('.acct-nav');
    if (!main) return;
    var over = (bar ? bar.offsetHeight : 0) +
      (nav && getComputedStyle(nav).position === 'sticky' ? nav.offsetHeight : 0);
    var top = main.getBoundingClientRect().top + window.scrollY - over;
    if (window.scrollY > top) window.scrollTo({ top: Math.max(0, top), behavior: 'instant' });
  }

  function sidebar(s) {
    var a = s.account;
    var lic = license();
    var name = a.display_name || a.email || t('signin_title');
    return h('div.acct-side',
      h('div.acct-side__in',
        h('div.acct-who',
          avatarOf(a),
          h('div.acct-who__text',
            h('p.acct-who__name#acct-name', { title: name }, name),
            a.email ? h('p.acct-who__mail', { title: a.email }, a.email) : null)),
        h('nav.acct-nav', { 'aria-label': t('tabs_label') },
          navTab('license', ICON.license, 'tab_license', 'nav_license'),
          RF ? navTab('referral', ICON.gift, 'tab_referral', 'nav_referral',
            h('span.acct-nav__rate', { 'aria-hidden': 'true' }, '30 %')) : null,
          navTab('machines', ICON.computer, 'machines', 'nav_machines',
            lic ? h('span.acct-nav__count', { 'aria-hidden': 'true' }, num(lic.machines.length) + '/' + num(lic.seats)) : null),
          // Invoices and the subscription live at Polar, the merchant of
          // record: this one leaves the page, and says so.
          s.billing && s.billing.portal ? [
            h('span.acct-nav__sep', { 'aria-hidden': 'true' }),
            h('button.acct-nav__item.acct-nav__item--ext#nav-billing', {
              type: 'button',
              'aria-busy': state.busy.portal ? 'true' : null,
              onclick: state.busy.portal ? null : function () { openPortal({ side: true }); }
            },
              svg(ICON.receipt),
              navLabel('nav_billing', 'nav_billing_short'),
              h('span.sr-only', ' ' + t('nav_billing_where')),
              h('span.acct-nav__ext', svg(ICON.external, 13)))
          ] : null),
        h('div.acct-side__foot',
          h('button.acct-nav__item.acct-nav__item--out#nav-signout', {
            type: 'button',
            'aria-busy': state.busy.signout ? 'true' : null,
            onclick: state.busy.signout ? null : signOut
          }, svg(ICON.signout), h('span.acct-nav__label', t('sign_out'))))
      )
    );
  }

  // The sidebar's long name; the phone's tab bar, the short one.
  function navLabel(long, short) {
    return [h('span.acct-nav__label', t(long)), h('span.acct-nav__short', t(short))];
  }

  function navTab(tab, icon, long, short, extra) {
    return h('a.acct-nav__item', {
      href: '#' + tab, id: 'tab-' + tab,
      'aria-current': state.tab === tab ? 'page' : null,
      onclick: function (ev) { goTab(ev, tab); }
    }, svg(icon), navLabel(long, short), extra);
  }

  var avatarCache = { key: null, el: null };

  function avatarOf(a) {
    var url = a.avatar_url && /^https:\/\//.test(a.avatar_url) ? a.avatar_url : null;
    var initials = (a.display_name || a.email || '?').split(/\s+/).map(function (w) { return w[0]; }).join('').slice(0, 2).toUpperCase();
    var key = url || 'initials:' + initials;
    if (avatarCache.key !== key) {
      avatarCache.key = key;
      avatarCache.el = url
        ? h('img.avatar', { src: url, alt: '', width: 40, height: 40, referrerpolicy: 'no-referrer', decoding: 'async' })
        : h('span.avatar.avatar--initials', { 'aria-hidden': 'true' }, initials);
      if (url) {
        avatarCache.el.addEventListener('error', function () {
          avatarCache.key = 'initials:' + initials;
          var fallback = h('span.avatar.avatar--initials', { 'aria-hidden': 'true' }, initials);
          if (avatarCache.el.parentNode) avatarCache.el.parentNode.replaceChild(fallback, avatarCache.el);
          avatarCache.el = fallback;
        });
      }
    }
    return avatarCache.el;
  }

  function picker(list) {
    return h('div.lic-pick', { role: 'group', 'aria-label': t('licenses') },
      list.map(function (l) {
        return h('button.lic-pick__btn', {
          type: 'button',
          'aria-pressed': String(l.id === (license() || {}).id),
          onclick: function () {
            state.current = l.id;
            state.flash = null;
            if (!state.history[l.id]) loadHistory(l.id, true);
            render();
          }
        }, kindName(l), h('span.lic-pick__id', l.id));
      })
    );
  }

  // A subscription is HangarAI Pro or HangarAI Max, by its product.
  function kindName(l) {
    if (l.kind === 'subscription' && isMax(l)) return t('kind_max');
    return t('kind_' + (STR.en['kind_' + l.kind] ? l.kind : 'unknown'));
  }

  function isMax(l) {
    return /^max_/.test(l.plan || '');
  }

  function planOf(s, product) {
    return (s.plans || []).filter(function (p) { return p.product === product; })[0] || null;
  }

  // The status line: a pill and a sentence, by kind and status.
  function statusOf(l, s) {
    var plan = l.plan && (s.plans || []).filter(function (p) { return p.product === l.plan; })[0];
    var planPrice = plan ? t(plan.interval === 'year' ? 'per_year' : 'per_month', { price: price(plan.price) }) : null;
    if (l.status === 'revoked') return { pill: pill(t('st_revoked'), 'danger'), text: t('revoked_text', { date: date(l.ends_at || Date.now()) }) };
    if (l.kind === 'trial') {
      if (l.status === 'ended') return { pill: pill(t('st_ended'), 'muted'), text: t('trial_ended', { date: date(l.ends_at) }) };
      var left = daysUntil(l.ends_at);
      return {
        pill: pill(t('st_trial'), 'yours'),
        text: left === 0 ? t('trial_today') : t(left === 1 ? 'trial_left_one' : 'trial_left', { n: left, date: date(l.ends_at) }),
        trial: Math.min(14, Math.max(1, 14 - left + 1))
      };
    }
    if (l.kind === 'subscription') {
      if (l.status === 'ended') return { pill: pill(t('st_ended'), 'muted'), text: t('ended_text', { date: date(l.ends_at) }) };
      if (l.status === 'canceling') return { pill: pill(t('st_canceling'), 'waiting'), text: t('canceling_text', { date: date(l.ends_at) }) };
      if (l.status === 'past_due') return { pill: pill(t('st_past_due'), 'waiting'), text: t('past_due_text') };
      return { pill: pill(t('st_active'), 'ok'), text: [l.renews_at ? t('renews', { date: date(l.renews_at) }) : null, planPrice].filter(Boolean).join(' · ') };
    }
    if (l.kind === 'lifetime') return { pill: pill(t('st_active'), 'ok'), text: t('lifetime_text', { date: date(l.created_at) }) };
    if (l.kind === 'free_grant') return { pill: pill(t('st_free'), 'muted'), text: t('free_text') };
    return { pill: pill(t('st_' + l.status) || l.status, 'muted'), text: '' };
  }

  function licensePanel(l, s) {
    var st = statusOf(l, s);
    var sub = l.kind === 'subscription' && l.status !== 'ended' && l.status !== 'revoked';
    var actions = [];
    if (sub && s.billing.portal) {
      actions.push(button(l.status === 'past_due' ? t('update_card') : t('manage'), {
        primary: l.status === 'past_due', busy: state.busy.portal, external: true, onclick: openPortal
      }));
    }
    if (s.billing.portal) actions.push(button(t('invoices'), { busy: state.busy.portal && !sub, external: true, onclick: openPortal }));

    var offers = null;
    if (l.kind === 'trial' || l.kind === 'free_grant' || (l.kind === 'subscription' && l.status === 'ended')) {
      offers = offersBlock(l, s, l.kind === 'trial' ? 'offers_trial' : l.kind === 'free_grant' ? 'offers_free' : 'offers_ended',
        l.kind === 'free_grant' ? 'offers_free_lede' : null);
    } else if (l.kind === 'subscription' && l.status === 'active' && !isMax(l) && s.billing.portal) {
      offers = upgradeBlock(l, s);
    }

    return h('section.panel.lic', { 'aria-labelledby': 'lic-title' },
      h('div.lic__top',
        h('div.lic__main',
          h('p.panel__eyebrow', t('license')),
          h('h2.lic__name#lic-title', { tabindex: '-1' }, kindName(l),
            l.plan && STR.en['plan_' + l.plan] ? h('span.chip', t('plan_' + l.plan)) : null),
          h('p.lic__status', st.pill, h('span', st.text))
        ),
        actions.length ? h('div.lic__actions', actions) : null
      ),
      st.trial ? trialBar(st.trial) : null,
      state.flash ? h('div.notice.notice--error#flash', { role: 'alert', tabindex: '-1' }, h('p', t(state.flash))) : null,
      offers,
      h('dl.lic__facts',
        h('div', h('dt', t('license_id')), h('dd', h('code', l.id))),
        h('div', h('dt', t('machines')), h('dd', h('a', { href: '#machines', onclick: function (ev) { goTab(ev, 'machines'); } },
          t('machines_used', { n: num(l.machines.length), seats: num(l.seats) })))),
        h('div', h('dt', t('since_label')), h('dd', date(l.created_at)))
      )
    );
  }

  function trialBar(day) {
    var pct = Math.round(day / 14 * 100);
    return h('div.trial',
      h('div.trial__bar', { 'aria-hidden': 'true' }, h('i', { style: 'width:' + pct + '%' })),
      h('p.trial__label', t('trial_day', { n: day }))
    );
  }

  function nonePanel(s) {
    return h('section.panel.lic', { 'aria-labelledby': 'lic-title' },
      h('p.panel__eyebrow', t('license')),
      h('h2.lic__name#lic-title', { tabindex: '-1' }, t('offers_none')),
      h('p.lic__lede', t('offers_none_lede')),
      state.flash ? h('div.notice.notice--error#flash', { role: 'alert', tabindex: '-1' }, h('p', t(state.flash))) : null,
      offersBlock(null, s, null, null)
    );
  }

  // What a year saves on twelve months of the same plan.
  function yearSaving(s, tier) {
    var month = planOf(s, tier + '_monthly');
    var year = planOf(s, tier + '_yearly');
    return month && year && month.price.currency === year.price.currency && month.price.amount * 12 > year.price.amount
      ? { amount: month.price.amount * 12 - year.price.amount, currency: year.price.currency } : null;
  }

  // Pro and Max, by the month and by the year: what the server has on
  // sale, two by two. A product this site no longer sells (Lifetime,
  // from a server before 3.0) is left out.
  function offersBlock(l, s, titleKey, ledeKey) {
    var plans = (s.plans || []).filter(function (p) {
      return A.PRODUCTS.indexOf(p.product) >= 0 && (p.interval === 'month' || p.interval === 'year');
    });
    var tiles = plans.map(function (p) {
      var tier = /^max_/.test(p.product) ? 'max' : 'pro';
      var per = p.interval === 'year' ? t('offer_year') : t('offer_month');
      var saving = p.interval === 'year' ? yearSaving(s, tier) : null;
      var credits = p.credits_per_month || (tier === 'max' ? 10000 : 3000);
      return h('button.offer' + (p.product === 'pro_yearly' ? '.offer--accent' : ''), {
        type: 'button',
        'aria-busy': state.busy['buy:' + p.product] ? 'true' : null,
        onclick: function () { buy(p.product, l); }
      },
        h('span.offer__name', STR.en['offer_' + p.product] ? t('offer_' + p.product) : p.product),
        h('span.offer__price', h('b', price(p.price)), ' ', per),
        h('span.offer__what', t('offer_credits', { n: num(credits) })),
        saving ? h('span.offer__extra', t('offer_save', { price: price(saving) })) : null
      );
    });
    return h('div.offers',
      titleKey ? h('h3.offers__title', t(titleKey)) : null,
      ledeKey ? h('p.offers__lede', t(ledeKey)) : null,
      h('div.offers__tiles', tiles),
      h('p.offers__fine', t('offers_fine'))
    );
  }

  // A Pro subscriber: Max, one step away. The plan is changed in Polar's
  // customer portal (the same subscription, prorated), never by a second
  // checkout, which the server refuses (already_subscribed).
  function upgradeBlock(l, s) {
    var month = planOf(s, 'max_monthly');
    var year = planOf(s, 'max_yearly');
    if (!month || !year) return null;
    var mine = planOf(s, l.plan);
    var allowance = l.credits && l.credits.allowance;
    return h('div.offers.upgrade',
      h('div.upgrade__text',
        h('h3.offers__title', t('upgrade_title')),
        h('p.offers__lede', t('upgrade_lede', {
          n: num(month.credits_per_month || 10000),
          m: num((mine && mine.credits_per_month) || (allowance && allowance.total) || 3000),
          k: num(month.build_machines || 5),
          month: price(month.price),
          year: price(year.price)
        }))),
      button(t('upgrade'), { primary: true, busy: state.busy.portal, external: true, cls: 'upgrade__btn', onclick: openPortal })
    );
  }

  function creditsPanel(l, s) {
    var c = l.credits || { allowance: null, purchased_left: 0, spent_this_period: [] };
    var a = c.allowance;
    var total = (a ? a.left : 0) + (c.purchased_left || 0);
    var ended = l.status === 'ended' || l.status === 'revoked';
    // What a credit buys, from the first hosted feature's rate as the
    // server has it: never a figure of this page's own.
    var rate = (s.rates || [])[0];
    var rateLine = null;
    var equiv = null;
    if (rate && rate.credits_per_unit > 0) {
      var feature = featureName(rate.feature).toLowerCase();
      rateLine = rate.credits_per_unit < 1
        ? t('per_credit', { n: num(1 / rate.credits_per_unit), unit: unitName(rate.unit), feature: feature })
        : t('per_unit', { feature: featureName(rate.feature), n: num(rate.credits_per_unit), unit: unitName(rate.unit) });
      if (rate.unit === 'minute' && total > 0) {
        var minutes = total / rate.credits_per_unit;
        equiv = minutes >= 120
          ? t('equiv_hours', { n: num(approxHours(minutes / 60)), feature: feature })
          : t('equiv_minutes', { n: num(Math.round(minutes)), feature: feature });
      }
    }
    var packs = ended ? [] : (s.packs || []);
    var spent = c.spent_this_period || [];

    return h('section.panel.credits', { 'aria-labelledby': 'credits-title' },
      h('div.panel__head',
        h('h2.panel__title#credits-title', t('credits')),
        rateLine ? h('p.panel__aside', rateLine) : null
      ),
      h('p.credits__total', h('span.credits__num', num(total)), h('span.credits__unit', t('credits_left'))),
      equiv ? h('p.credits__equiv', equiv) : null,
      a ? h('div.meter-row',
        h('div.meter-row__label', h('span', l.kind === 'trial' ? t('allowance_trial') : t('allowance')),
          h('span.meter-row__value', t('allowance_left', { left: num(a.left), total: num(a.total) }))),
        h('div.meter', { 'aria-hidden': 'true' }, h('i', { style: 'width:' + (a.total ? Math.round(a.left / a.total * 100) : 0) + '%' })),
        h('p.meter-row__fine', l.kind === 'trial' ? t('trial_until', { date: date(a.resets_at) }) : t('resets', { date: date(a.resets_at) }))
      ) : null,
      h('div.meter-row',
        h('div.meter-row__label', h('span', t('purchased')),
          h('span.meter-row__value', t('purchased_left', { n: num(c.purchased_left || 0) }))),
        h('p.meter-row__fine', ended ? t('purchased_ended') : t('purchased_fine'))
      ),
      h('div.spent',
        h('h3.spent__title', t('spent_period')),
        spent.length
          ? h('ul.spent__list', spent.map(function (x) {
            var r = (s.rates || []).filter(function (rr) { return rr.feature === x.feature; })[0];
            return h('li', h('span', featureName(x.feature)),
              h('span.spent__n', r ? num(x.units) + ' ' + unitName(r.unit) + ' · ' : '', num(x.credits) + ' ' + t('credits').toLowerCase()));
          }))
          : h('p.spent__none', t('spent_none'))
      ),
      packs.length ? h('div.packs',
        packs.map(function (p) {
          return button(t('buy_pack', { n: num(p.credits) }) + ' · ' + price(p.price), {
            small: true, busy: state.busy['buy:' + p.product], onclick: function () { buy(p.product, l); }
          });
        }),
        h('p.packs__fine', t('packs_fine'))
      ) : null,
      paygBlock(l)
    );
  }

  function paygBlock(l) {
    var p = payg(l);
    if (!p) return null;
    var on = !!(p.enabled && p.available);
    var lede = p.price ? t('payg_lede', { price: price(p.price), n: num(p.price.credits) }) : null;
    var limits = p.limits || { presets: [], min: 100, max: 50000 };
    var current = p.limit && p.limit.amount;
    var custom = state.paygCustom || (current && limits.presets.indexOf(current) < 0);
    var body = null;
    if (!p.available) {
      body = h('p.payg__note', t(STR.en['payg_r_' + p.reason] ? 'payg_r_' + p.reason : 'err_payg_unavailable'));
    } else if (!on) {
      body = h('p.payg__note', t(p.setup === 'checkout' ? 'payg_checkout_note' : 'payg_off_note'));
    } else {
      var pct = p.limit && p.limit.amount ? Math.min(100, Math.round((p.spent ? p.spent.amount : 0) / p.limit.amount * 100)) : 0;
      body = [
        h('div.payg__limit',
          h('p.payg__label#payg-limit', t('payg_limit')),
          h('div.chips', { role: 'group', 'aria-labelledby': 'payg-limit' },
            limits.presets.map(function (amount) {
              return h('button.chip-btn', {
                type: 'button', 'aria-pressed': String(!custom && current === amount),
                onclick: function () { state.paygCustom = false; if (current !== amount) setLimit(l, amount); }
              }, price({ amount: amount, currency: 'EUR' }));
            }),
            h('button.chip-btn', {
              type: 'button', 'aria-pressed': String(!!custom),
              onclick: function () { state.paygCustom = true; after = '#payg-custom'; render(); }
            }, t('payg_other'))
          ),
          custom ? h('form.payg__custom', {
            onsubmit: function (ev) {
              ev.preventDefault();
              var euros = Math.round(parseFloat(document.getElementById('payg-custom').value.replace(',', '.')) * 100);
              if (!(euros >= limits.min && euros <= limits.max)) {
                state.paygError = 'err_bad_request';
                return render();
              }
              setLimit(l, euros);
            }
          },
            h('label', { 'for': 'payg-custom' }, t('payg_custom', { min: price({ amount: limits.min, currency: 'EUR' }), max: price({ amount: limits.max, currency: 'EUR' }) })),
            h('div.payg__row',
              h('input#payg-custom', {
                type: 'number', inputmode: 'decimal', min: limits.min / 100, max: limits.max / 100, step: 1,
                value: current ? current / 100 : null
              }),
              button(t('payg_set'), { small: true, submit: true, busy: state.busy.payg })
            )
          ) : null
        ),
        h('div.meter-row.payg__use',
          h('div.meter-row__label', h('span', t('payg_used', { spent: price(p.spent || { amount: 0, currency: 'EUR' }), limit: price(p.limit) })),
            p.credits ? h('span.meter-row__value', t('payg_credits', { n: num(p.credits) })) : null),
          h('div.meter', { 'aria-hidden': 'true' }, h('i', { style: 'width:' + pct + '%' })),
          p.resets_at ? h('p.meter-row__fine', t('payg_resets', { date: date(p.resets_at) })) : null
        )
      ];
    }
    return h('div.payg' + (on ? '.is-on' : ''),
      h('div.payg__head',
        h('div.payg__text',
          h('h3.payg__title#payg-title', t('payg_title')),
          lede ? h('p.payg__lede#payg-lede', lede) : null
        ),
        p.available ? h('button.switch#payg-switch', {
          type: 'button', role: 'switch', 'aria-checked': String(on),
          'aria-labelledby': 'payg-title', 'aria-describedby': lede ? 'payg-lede' : null,
          'aria-busy': state.busy.payg || state.busy['buy:payg'] ? 'true' : null,
          onclick: function () { if (!state.busy.payg) togglePayg(l); }
        }, h('i')) : null
      ),
      body,
      state.paygError ? h('p.payg__error', { role: 'alert' }, t(state.paygError)) : null
    );
  }

  function machinesPanel(l) {
    var free = Math.max(0, l.seats - l.machines.length);
    return h('section.panel.machines', { 'aria-labelledby': 'machines-title' },
      h('div.panel__head',
        h('h2.panel__title#machines-title', { tabindex: '-1' }, t('machines_active')),
        h('p.panel__aside', t('machines_used', { n: num(l.machines.length), seats: num(l.seats) }))
      ),
      h('div.seats', { 'aria-hidden': 'true' },
        Array.apply(null, Array(Math.min(l.seats, 12))).map(function (_, i) {
          return h('i' + (i < l.machines.length ? '.on' : ''));
        })),
      l.machines.length
        ? h('ul.machines__list', l.machines.map(function (m) {
          var name = machineName(m);
          var meta = [osName(m.os), t('seen', { when: ago(m.last_seen_at) })].filter(Boolean).join(' · ');
          return h('li.machine',
            h('span.machine__icon', svg(ICON.computer, 18)),
            h('div.machine__text',
              h('p.machine__name', name),
              h('p.machine__meta', meta),
              h('p.machine__meta', t('activated', { date: date(m.activated_at) }))
            ),
            button(t('deactivate'), {
              small: true, label: t('deactivate_label', { name: name }),
              busy: state.busy['seat:' + m.id], onclick: function () { deactivate(l, m); }
            })
          );
        }).concat(free ? h('li.machine.machine--free',
          h('span.machine__icon', svg(ICON.plus, 18)),
          h('div.machine__text',
            h('p.machine__name', free === 1 ? t('seat_free_one') : t('seat_free', { n: num(free) })),
            h('p.machine__meta', t('seat_free_text')))) : []))
        : h('p.machines__empty', t('machines_empty')),
      l.machines.length ? h('p.panel__fine', t('machines_fine')) : null
    );
  }

  function historyLabel(e) {
    if (e.type === 'spend') return e.payg ? t('h_payg', { feature: featureName(e.feature) }) : featureName(e.feature);
    if (e.type === 'removal') return e.source === 'refund' ? t('h_refund') : t('h_admin');
    if (e.source === 'allowance') return t('h_allowance');
    if (e.source === 'trial') return t('h_trial');
    if (e.source === 'lifetime') return t('h_lifetime');
    if (e.source === 'gift') return t('h_gift');
    if (e.source === 'purchase') {
      var n = /^credits_(\d+)$/.exec(e.product || '');
      return n ? t('h_purchase', { n: num(+n[1]) }) : t('h_purchase_any');
    }
    return e.source || e.type;
  }

  function historyPanel(l) {
    var hist = state.history[l.id] || { entries: [], loading: true };
    var rates = (state.summary.rates || []);
    var body;
    if (!hist.done && hist.loading) {
      body = h('div.ledger-wait', { role: 'status' }, h('span.spinner', { 'aria-hidden': 'true' }), h('span.sr-only', t('loading')));
    } else if (hist.error && !hist.entries.length) {
      body = h('div.notice.notice--error', { role: 'alert' }, h('p', t('history_error')),
        h('p', button(t('retry'), { small: true, onclick: function () { loadHistory(l.id, true); } })));
    } else if (!hist.entries.length) {
      body = h('p.ledger-empty', t('history_empty'));
    } else {
      body = h('table.ledger',
        h('caption.sr-only', t('history_caption')),
        h('thead', h('tr', h('th', { scope: 'col' }, t('col_date')), h('th', { scope: 'col' }, t('col_what')), h('th.num', { scope: 'col' }, t('col_credits')))),
        h('tbody', hist.entries.map(function (e, i) {
          var r = e.type === 'spend' ? rates.filter(function (rr) { return rr.feature === e.feature; })[0] : null;
          var sign = e.credits > 0 ? '+' : e.credits < 0 ? '−' : '';
          return h('tr' + (e.credits > 0 ? '.is-grant' : ''), { id: 'ledger-row-' + i, tabindex: '-1' },
            h('td.ledger__date', h('time', { datetime: new Date(e.at).toISOString() }, shortDate(e.day ? Date.parse(e.day + 'T12:00:00Z') : e.at))),
            h('td.ledger__what', historyLabel(e), r && e.units ? h('span.ledger__units', num(e.units) + ' ' + unitName(r.unit)) : null),
            h('td.num.ledger__credits', sign + num(Math.abs(e.credits)))
          );
        }))
      );
    }
    return h('section.panel.history', { 'aria-labelledby': 'history-title' },
      h('div.panel__head', h('h2.panel__title#history-title', t('history'))),
      body,
      hist.next ? h('div.ledger-more', button(t('history_more'), { small: true, busy: hist.loading, onclick: function () { loadHistory(l.id, false); } })) : null,
      hist.error && hist.entries.length ? h('p.panel__fine', { role: 'alert' }, t('history_error')) : null
    );
  }

  function downloadsPanel() {
    var d = state.downloads;
    var you = detectOs();
    return h('section.panel.downloads', { 'aria-labelledby': 'downloads-title' },
      h('div.panel__head',
        h('h2.panel__title#downloads-title', t('downloads')),
        d && d.version ? h('p.panel__aside', t('version', { v: d.version })) : null
      ),
      !d
        ? h('ul.dl', { 'aria-hidden': 'true' }, [0, 1, 2].map(function () {
          return h('li.dl__item.dl__item--bone', h('div.bone.bone--line'), h('div.bone.bone--btn'));
        }))
        : d.plats.length
        ? h('ul.dl', d.plats.map(function (p) {
          return h('li.dl__item' + (p.os === you ? '.is-you' : ''),
            h('p.dl__os', p.name, p.os === you ? h('span.plat__you', t('this_computer')) : null),
            h('a.btn.btn--sm' + (p.os === you ? '.btn--primary' : ''), { href: p.href },
              t('dl_get', { ext: '' }).trim(), ' ', h('span.ext', p.label)),
            p.alts.length ? h('p.dl__alts', p.alts.map(function (alt) { return h('a', { href: alt.href }, alt.label); })) : null
          );
        }))
        : h('p.panel__fine', t('downloads_fallback'), ' ', h('a', { href: '../#download' }, '../#download')),
      d && d.all ? h('p.panel__fine', h('a', { href: d.all }, t('all_files'))) : null
    );
  }

  function keyPanel() {
    var err = state.keyError;
    return h('section.panel.addkey',
      h('details', { open: err || state.busy.key ? true : null },
        h('summary', t('key_title')),
        h('form.addkey__form', {
          novalidate: true,
          onsubmit: function (ev) {
            ev.preventDefault();
            var input = document.getElementById('key-input');
            claim(input.value.trim());
          }
        },
          h('label', { 'for': 'key-input' }, t('key_label')),
          h('div.addkey__row',
            h('input#key-input', {
              type: 'text', name: 'key', autocomplete: 'off', autocapitalize: 'characters', spellcheck: 'false',
              placeholder: 'HANGAR-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX',
              'aria-invalid': err ? 'true' : null, 'aria-describedby': err ? 'key-error' : null,
              value: state.keyValue || null,
              oninput: function (ev) { state.keyValue = ev.target.value; }
            }),
            button(t('key_add'), { submit: true, busy: state.busy.key })
          ),
          err ? h('p.addkey__error#key-error', { role: 'alert' }, t(err)) : null
        )
      )
    );
  }

  function detectOs() {
    var ua = navigator.userAgent;
    if (/Android|iPhone|iPad|iPod/.test(ua) || (/Macintosh/.test(ua) && navigator.maxTouchPoints > 1)) return null;
    return /Mac OS X|Macintosh/.test(ua) ? 'macos' : /Windows/.test(ua) ? 'windows' : /Linux|X11|CrOS/.test(ua) ? 'linux' : null;
  }

  // The downloads are the home page's: its cards are bumped at each
  // release (site/README.md), so this reads them rather than keeping a
  // second copy of every address.
  function readDownloads() {
    return fetch('../', { cache: 'no-cache' }).then(function (res) {
      if (!res.ok) throw new Error(res.status);
      return res.text();
    }).then(function (html) {
      var doc = new DOMParser().parseFromString(html, 'text/html');
      var title = doc.getElementById('download-title');
      var version = title && (/(\d+\.\d+\.\d+)/.exec(title.textContent) || [])[1];
      var plats = Array.prototype.map.call(doc.querySelectorAll('.plat[data-os]'), function (card) {
        var get = card.querySelector('.plat__get');
        var ext = get && get.querySelector('.ext');
        var label = get ? get.textContent.replace(/\s+/g, ' ').trim() : '';
        return {
          os: card.dataset.os,
          name: (card.querySelector('.plat__os') || {}).textContent || osName(card.dataset.os),
          href: get && get.getAttribute('href'),
          label: ext ? ext.textContent.trim() : label,
          alts: Array.prototype.map.call(card.querySelectorAll('.plat__alts a'), function (a) {
            return { href: a.getAttribute('href'), label: a.textContent.trim() };
          })
        };
      }).filter(function (p) { return p.href && /^https:\/\//.test(p.href); });
      var all = doc.querySelector('.after a[href*="releases/latest"]');
      state.downloads = { version: version, plats: plats, all: all && all.getAttribute('href') };
      render();
    }).catch(function () {
      state.downloads = { version: null, plats: [], all: null };
      render();
    });
  }

  /* the dialog and the toast ------------------------------------- */

  function dialog(o) {
    var dlg = document.getElementById('dlg');
    if (!dlg || typeof dlg.showModal !== 'function') {
      return Promise.resolve(o.cancel ? window.confirm(o.title + '\n\n' + o.text) : (window.alert(o.text), true));
    }
    var back = document.activeElement;
    dlg.textContent = '';
    var confirm = button(o.confirm, { primary: !o.danger, cls: o.danger ? 'btn--danger' : '' });
    var cancel = o.cancel ? button(o.cancel, {}) : null;
    dlg.appendChild(h('div.dlg__body', h('h2.dlg__title#dlg-title', o.title), h('p.dlg__text#dlg-text', o.text)));
    dlg.appendChild(h('div.dlg__actions', cancel, confirm));
    dlg.setAttribute('aria-labelledby', 'dlg-title');
    dlg.setAttribute('aria-describedby', 'dlg-text');
    return new Promise(function (resolve) {
      function close(value) {
        dlg.close();
        resolve(value);
        if (back && document.contains(back)) back.focus();
      }
      confirm.addEventListener('click', function () { close(true); });
      if (cancel) cancel.addEventListener('click', function () { close(false); });
      dlg.oncancel = function (ev) { ev.preventDefault(); close(false); };
      dlg.showModal();
      (cancel || confirm).focus();
    });
  }

  var toastTimer = null;
  function toast(text) {
    var el = document.getElementById('toast');
    var live = document.getElementById('live');
    // The toast is for the eyes; the live region, always there, says it.
    if (live) {
      live.textContent = '';
      setTimeout(function () { live.textContent = text; }, 60);
    }
    if (!el) return;
    el.textContent = text;
    el.hidden = false;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(function () { el.hidden = true; }, 5000);
  }

  /* drawing -------------------------------------------------------- */

  function mockBanner() {
    if (!api || !api.mock) return null;
    function pick(label, param, names, current) {
      return h('label', label, ' ',
        h('select', {
          onchange: function (ev) {
            var url = new URL(location.href);
            url.searchParams.set(param, ev.target.value);
            location.assign(url.toString());
          }
        }, names.map(function (name) {
          return h('option', { value: name, selected: name === current ? true : null }, name);
        })));
    }
    return h('div.mock', { role: 'note' },
      h('b', t('mock_label')), h('span', t('mock_text')),
      pick(t('mock_account'), 'as', api.scenarios, api.scenario),
      api.refScenarios && state.tab === 'referral' ? pick(t('tab_referral'), 'ref', api.refScenarios, api.refScenario) : null
    );
  }

  var lastKind = null;
  var lastTab = null;

  function render() {
    var app = document.getElementById('app');
    if (!app) return;
    var view = state.view === 'ready' && state.summary ? viewReady()
      : state.view === 'down' ? viewDown()
        : state.view === 'loading' ? (seen() ? viewLoading() : viewWait())
          : viewSignIn();
    // Keep the key being typed, and the focus on the button that was
    // pressed when it is drawn again.
    var focused = document.activeElement;
    var focusId = focused && app.contains(focused) && focused.id ? focused.id : null;
    var caret = focusId && focused.tagName === 'INPUT' ? [focused.selectionStart, focused.selectionEnd] : null;
    var kind = state.view === 'loading' ? (seen() ? 'loading' : 'wait')
      : state.view === 'signingin' ? 'signedout' : state.view;
    // A new view rises whole; another tab of the same one, beside the
    // sidebar, which stays put.
    var page = view.querySelector('.acct-page');
    if (kind !== lastKind) view.classList.add('enter');
    else if (kind === 'ready' && state.tab !== lastTab && page) page.classList.add('enter-tab');
    lastKind = kind;
    lastTab = state.tab;
    app.textContent = '';
    var banner = mockBanner();
    if (banner) app.appendChild(banner);
    // The app (the account, and its shape while it loads) takes the
    // page's width; the sign-in and the rest sit in the middle.
    app.appendChild(view.classList.contains('acct-app') ? view : h('div.wrap.acct-wrap', view));
    // On a phone the tab bar scrolls sideways: the tab on screen in view.
    var nav = app.querySelector('.acct-nav');
    var cur = nav && nav.querySelector('[aria-current="page"]');
    if (cur && nav.scrollWidth > nav.clientWidth) {
      var left = cur.getBoundingClientRect().left - nav.getBoundingClientRect().left + nav.scrollLeft;
      if (left < nav.scrollLeft || left + cur.offsetWidth > nav.scrollLeft + nav.clientWidth) {
        nav.scrollLeft = Math.max(0, left - 16);
      }
    }
    var target = after ? app.querySelector(after) : focusId ? document.getElementById(focusId) : null;
    after = null;
    if (target) {
      target.focus({ preventScroll: !!focusId });
      if (caret && target.setSelectionRange) target.setSelectionRange(caret[0], caret[1]);
      if (!focusId && !reduced && target.scrollIntoView) target.scrollIntoView({ block: 'nearest' });
    }
    if (RF && state.view === 'ready' && state.tab === 'referral') RF.after(app.querySelector('.rf'));
  }

  /* ── 5. start ──────────────────────────────────────────────── */

  // What account/referral.js draws "Invite & earn" with: this page's
  // helpers, formats and state.
  var RF = RF_MOD ? RF_MOD.create({
    h: h, t: t, num: num, nbsp: nbsp, date: date, shortDate: shortDate, ago: ago, button: button,
    state: state, render: render, errorKey: errorKey, toast: toast, lang: lang,
    api: function () { return api; },
    reload: function () { return load(); },
    focus: function (selector) { after = selector; },
    announce: function (text) {
      var live = document.getElementById('live');
      if (!live) return;
      live.textContent = '';
      setTimeout(function () { live.textContent = text; }, 60);
    }
  }) : null;

  window.addEventListener('popstate', function () { showTab(tabOf(location.hash)); });
  window.addEventListener('hashchange', function () { showTab(tabOf(location.hash)); });

  function loadMock() {
    return new Promise(function (resolve, reject) {
      var s = document.createElement('script');
      s.src = 'mock.js';
      s.onload = resolve;
      s.onerror = reject;
      document.head.appendChild(s);
    });
  }

  function start() {
    api = MOCK
      ? window.HangarMock.create({ as: params.get('as'), ref: params.get('ref'), slow: params.get('slow') === '1', signin: params.get('signin') })
      : A.create();
    if (Lang) Lang.on(render);
    readDownloads();

    var code = params.get('code');
    var error = params.get('error');
    if ((code || error) && params.get('state') !== null) {
      return finishSignIn(code, params.get('state'), error);
    }
    if (params.get('checkout') === 'done') {
      cleanUrl(['checkout']);
      toast(t('thanks'));
      load().then(function () { setTimeout(function () { if (state.view === 'ready') load({ fresh: true }); }, 4000); });
      return;
    }
    load();
  }

  (MOCK ? loadMock() : Promise.resolve()).then(start, function () {
    state.view = 'down';
    render();
  });
})();
