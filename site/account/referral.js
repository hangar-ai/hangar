/* ═══════════════════════════════════════════════════════════════
   HangarAI account: "Invite & earn", hangarai.dev/account#referral

   The referral program, for the signed-in account: what it earned and
   what is on its way, a simulator, the link (copy, QR code, share, a
   code of one's own), how it works, the funnel from visits to paying
   customers with twelve weeks of them, the referrals (anonymous), the
   milestones, the payouts and their statements, the payout details.
   The server's side is docs/native/portal.md, "The referral program";
   ?mock=1 runs it against account/mock.js (&ref= picks the story).

   account.js owns the page: it hands create() its helpers (h, the
   formats, the dialog, the toast, render) and draws view() under the
   "Invite & earn" tab. Like the rest of the page, everything is drawn
   with DOM calls and textContent: nothing the server sends becomes
   HTML. The motion (assets' GSAP, when it loaded) plays once, the
   first time the tab is drawn with its figures, and never with
   reduced motion.

     1. strings, English and French
     2. countries and IBANs
     3. the view, part by part
     4. the motion
   ═══════════════════════════════════════════════════════════════ */
(function () {
  'use strict';

  /* ── 1. strings ────────────────────────────────────────────── */

  var STR = {
    en: {
      tab_license: 'License & credits',
      tab_referral: 'Invite & earn',
      tabs_label: 'Account sections',
      rf_signin_title: 'Invite friends, earn 30 % for life',
      rf_signin_lede: 'Sign in to get your link. You earn 30 % of what each friend pays, for as long as they pay, and they get 1,000 bonus credits.',
      rf_loading: 'Loading Invite & earn…',
      rf_error: 'Invite & earn couldn’t be loaded.',

      rf_earned: 'You have earned',
      rf_first_label: 'Your first paying friend brings you',
      rf_per_month_life: 'a month, for life',
      rf_first_lede: 'Share your link below. Your friend gets {n} bonus credits when they subscribe, and you get {rate} % of every payment they make, for as long as they pay.',
      rf_monthly: '+{amount} a month',
      rf_monthly_fine: 'from the {n} friends paying now, for as long as they pay',
      rf_monthly_fine_one: 'from the friend paying now, for as long as they pay',
      rf_next_scheduled: '{amount} will be transferred on {date}.',
      rf_next_below: '{amount} payable. {left} more and it goes out on the 1st of the month.',
      rf_next_profile: '{amount} is ready. Add your payout details to receive it on {date}.',
      rf_next_hold: 'Nothing payable yet: {amount} is in its {days}-day hold.',
      rf_next_none: 'Payouts go out on the 1st of each month, from {min}.',
      rf_release: '{amount} clears the hold on {date}.',
      rf_meter_label: 'Toward the next transfer',
      rf_meter_value: '{amount} of {min}',
      rf_meter_ready: 'Transfer scheduled',
      rf_meter_waiting: 'Waiting for your payout details',
      rf_add_details: 'Add payout details',
      rf_preview: 'What your friends see',
      rf_preview_who: '{name} invited you',
      rf_preview_friend: 'A friend invited you',
      rf_preview_gift: '{n} bonus credits when you subscribe',
      rf_preview_cta: 'Try HangarAI free',
      rf_pending: 'In the 30-day hold',
      rf_pending_fine: 'Refundable for {days} days, then payable.',
      rf_payable: 'Payable',
      rf_payable_fine: 'Paid on the 1st, from {min}.',
      rf_paid: 'Paid out',
      rf_paid_fine: 'Transferred to you so far.',

      rf_link_title: 'Your link',
      rf_link_lede: 'Anyone who subscribes after opening it is yours, for life.',
      rf_copy: 'Copy',
      rf_copied: 'Copied',
      rf_copy_label: 'Copy your link',
      rf_copied_toast: 'Your link is copied.',
      rf_copy_failed: 'This browser didn’t let the page copy. The link is selected: copy it with your keyboard.',
      rf_customize: 'Customize',
      rf_code_label: 'Your code',
      rf_code_hint: '{min} to {max} letters, digits or hyphens. Links you already shared keep working.',
      rf_code_bad: 'Use {min} to {max} lowercase letters, digits or single hyphens, not at either end.',
      rf_code_save: 'Save',
      rf_code_saved: 'Your link is now {link}',
      err_code_invalid: 'Use 3 to 20 lowercase letters, digits or single hyphens, not at either end.',
      err_code_forbidden: 'That code is reserved. Pick another one.',
      err_code_taken: 'That code is taken. Try another one.',
      rf_qr: 'QR code',
      rf_qr_label: 'QR code of {link}',
      rf_qr_download: 'Download PNG',
      rf_share: 'Share',
      rf_share_on: 'Share on {where}',
      rf_share_more: 'More…',
      rf_share_email: 'Email',
      rf_discord_toast: 'Message copied: paste it in a Discord channel.',
      rf_message: 'I run my Claude Code and Codex agents side by side with HangarAI. Try it with my link: you get 1,000 bonus credits when you subscribe. {link}',
      rf_subject: 'Try HangarAI',

      rf_how: 'How it works',
      rf_step1: 'Share your link',
      rf_step1_text: 'On a stream, in a video, a post, a README or a message to your team.',
      rf_step2: 'Your friend gets {n} credits',
      rf_step2_text: 'Bonus credits with their first payment, on top of their plan.',
      rf_step3: 'You get {rate} %, for life',
      rf_step3_text: 'Of what HangarAI receives from them, after VAT and payment fees: every payment, as long as they pay.',
      rf_how_fine: 'Each commission is held {days} days, the time for a refund, then paid on the 1st of the month from {min}, by bank transfer or PayPal.',
      rf_terms: 'Terms',
      rf_faq: 'Questions',

      rf_sim: 'What could it be?',
      rf_sim_plan: 'Their plan',
      rf_sim_label: 'Friends who subscribe',
      rf_sim_people: '{n} friends',
      rf_sim_people_one: '1 friend',
      rf_sim_month: 'a month',
      rf_sim_year: '{amount} a year, for as long as they pay',
      rf_sim_fine: 'With {plan} monthly: {cut} a month per friend, {rate} % of what HangarAI keeps after VAT and fees. An estimate, not a promise.',

      rf_funnel: 'From visit to customer',
      rf_funnel_aside: 'Since your link exists',
      rf_visits: 'Visits',
      rf_trials: 'Trials',
      rf_customers: 'Customers',
      rf_visits_fine: 'opened your link',
      rf_trials_fine: 'started the 14-day trial',
      rf_customers_fine: 'paid at least once',
      rf_conv: '{pct} of visits',
      rf_conv_trials: '{pct} of trials',
      rf_weeks: 'Last 12 weeks',
      rf_week_tip: 'Week of {date}: {n}',
      rf_weeks_caption: '{what} per week, the last 12 weeks',
      rf_week_col: 'Week of',
      rf_funnel_empty: 'Nothing yet. A visit shows here a few minutes after someone opens your link.',

      rf_people: 'Your referrals',
      rf_people_aside: '{n} customers · {m} on a trial',
      rf_people_aside_one: '1 customer · {m} on a trial',
      rf_p_pro_monthly: 'A Pro subscriber, monthly',
      rf_p_pro_yearly: 'A Pro subscriber, yearly',
      rf_p_max_monthly: 'A Max subscriber, monthly',
      rf_p_max_yearly: 'A Max subscriber, yearly',
      rf_p_lifetime: 'A Lifetime owner',
      rf_p_credits: 'A credits buyer',
      rf_p_trial: 'On the 14-day trial',
      rf_p_ended: 'A former subscriber',
      rf_p_refunded: 'Refunded',
      rf_since: 'since {when}',
      rf_started: 'started {when}',
      rf_per_month: '+{amount} / month',
      rf_once: '{amount} once',
      rf_so_far: '{amount} so far',
      rf_nothing_yet: 'Nothing yet',
      rf_people_more: 'Show all {n}',
      rf_people_less: 'Show fewer',
      rf_people_beyond: 'And {n} more, not listed.',
      rf_people_empty: 'Your first friend will bring you ~{amount} a month, for life.',
      rf_people_empty_text: 'They show here as soon as they start a trial with your link, never by name: who they are stays theirs.',
      rf_copy_link: 'Copy your link',

      rf_badges: 'Milestones',
      rf_badge_1: 'First customer',
      rf_badge_n: '{n} customers',
      rf_badge_reached: 'Reached {date}',
      rf_badge_togo: '{n} to go',
      rf_badge_locked: 'Locked',
      rf_badges_fine: 'Markers along the way. The rate stays {rate} % from the first customer on.',

      rf_money: 'Payouts and statements',
      rf_payouts: 'Transfers',
      rf_payouts_count: '{n} so far',
      rf_payouts_empty: 'No transfer yet. The first one goes out on the 1st of a month once {min} are payable.',
      rf_statements: 'Statements',
      rf_statements_count: '{n} months',
      rf_statements_empty: 'Your first statement appears the month your first commission is recorded.',
      rf_statements_error: 'The statements couldn’t be loaded.',
      rf_st_earned: '{amount} earned',
      rf_st_cancelled: '{amount} cancelled',
      rf_st_paid: '{amount} paid on {date}',
      rf_st_open: 'View',
      rf_st_open_label: 'View the statement of {month}',
      rf_st_csv: 'CSV',
      rf_st_csv_label: 'Download the statement of {month} as CSV',
      rf_st_title: 'Statement · {month}',
      rf_st_payee: 'Paid to {name} ({country})',
      rf_st_date: 'Date',
      rf_st_ref: 'Referral',
      rf_st_what: 'What',
      rf_st_net: 'Net',
      rf_st_commission: 'Commission',
      rf_st_status: 'Status',
      rf_st_total: 'Earned this month',
      rf_st_total_cancelled: 'Cancelled this month',
      rf_st_payout: 'Transferred on {date} · ref. {ref}',
      rf_st_no_payout: 'No transfer this month.',
      rf_st_print: 'Print or save as PDF',
      rf_st_close: 'Close',
      rf_st_issuer: 'HangarAI referral program · hangarai.dev',
      rf_s_pending: 'In hold',
      rf_s_payable: 'Payable',
      rf_s_paid: 'Paid',
      rf_s_cancelled: 'Cancelled',
      rf_k_payment: '{product} payment',
      rf_k_refund: '{product} refund',
      rf_prod_pro_monthly: 'Pro, monthly',
      rf_prod_pro_yearly: 'Pro, yearly',
      rf_prod_max_monthly: 'Max, monthly',
      rf_prod_max_yearly: 'Max, yearly',
      rf_prod_lifetime: 'Lifetime',
      rf_prod_credits: 'Credits',
      rf_payouts_next: 'Next payout',

      rf_payout: 'Payout details',
      rf_payout_lede: 'Where your commissions go, once a month.',
      rf_payout_lock: 'Encrypted on our server and used only to pay you. The account number is never shown again in full.',
      rf_payout_none: 'Add them once, and every payout goes out on its own.',
      rf_payout_edit: 'Edit',
      rf_legal_name: 'Legal name',
      rf_legal_hint: 'The name on the account the money goes to: yours, or your company’s.',
      rf_country: 'Country',
      rf_country_pick: 'Choose…',
      rf_method: 'Paid by',
      rf_m_iban: 'Bank transfer',
      rf_m_paypal: 'PayPal',
      rf_iban: 'IBAN',
      rf_iban_ok: 'Checks out',
      rf_bic: 'BIC',
      rf_optional: 'optional',
      rf_paypal_email: 'PayPal email',
      rf_accept: 'I accept the {terms}.',
      rf_accept_terms: 'affiliate terms',
      rf_payout_save: 'Save payout details',
      rf_payout_saved: 'Payout details saved.',
      rf_tax: 'Commissions are income: declaring them where you live is up to you.',
      rf_f_name: 'Enter the name the money goes to.',
      rf_f_country: 'Choose a country.',
      err_iban_invalid: 'That IBAN doesn’t check out. Compare it with the one your bank shows.',
      err_paypal_invalid: 'That isn’t an email address.',
      err_terms_needed: 'Accept the terms to receive payouts.',
      rf_updated: 'Updated {date}'
    },
    fr: {
      tab_license: 'Licence et crédits',
      tab_referral: 'Parrainer et gagner',
      tabs_label: 'Sections du compte',
      rf_signin_title: 'Parraine, gagne 30 % à vie',
      rf_signin_lede: 'Connecte-toi pour obtenir ton lien. Tu touches 30 % de ce que paie chaque filleul, tant qu’il paie, et il reçoit 1 000 crédits offerts.',
      rf_loading: 'Chargement du parrainage…',
      rf_error: 'Le parrainage n’a pas pu être chargé.',

      rf_earned: 'Tu as gagné',
      rf_first_label: 'Ton premier filleul abonné te rapporte',
      rf_per_month_life: 'par mois, à vie',
      rf_first_lede: 'Partage ton lien ci-dessous. Ton filleul reçoit {n} crédits offerts à l’abonnement, et tu touches {rate} % de chacun de ses paiements, tant qu’il paie.',
      rf_monthly: '+{amount} par mois',
      rf_monthly_fine: 'grâce aux {n} filleuls qui paient en ce moment, tant qu’ils paient',
      rf_monthly_fine_one: 'grâce au filleul qui paie en ce moment, tant qu’il paie',
      rf_next_scheduled: '{amount} te seront versés le {date}.',
      rf_next_below: '{amount} à verser. Encore {left} et le virement part le 1er du mois.',
      rf_next_profile: '{amount} sont prêts. Ajoute tes coordonnées de versement pour les recevoir le {date}.',
      rf_next_hold: 'Rien à verser pour l’instant : {amount} sont dans leur délai de {days} jours.',
      rf_next_none: 'Les versements partent le 1er de chaque mois, dès {min}.',
      rf_release: '{amount} sortent du délai le {date}.',
      rf_meter_label: 'Vers le prochain virement',
      rf_meter_value: '{amount} sur {min}',
      rf_meter_ready: 'Virement programmé',
      rf_meter_waiting: 'En attente de tes coordonnées',
      rf_add_details: 'Ajouter mes coordonnées',
      rf_preview: 'Ce que voient tes amis',
      rf_preview_who: '{name} t’invite',
      rf_preview_friend: 'Un ami t’invite',
      rf_preview_gift: '{n} crédits offerts à l’abonnement',
      rf_preview_cta: 'Essayer HangarAI gratuitement',
      rf_pending: 'Dans le délai de 30 jours',
      rf_pending_fine: 'Remboursables pendant {days} jours, puis à verser.',
      rf_payable: 'À verser',
      rf_payable_fine: 'Versés le 1er du mois, dès {min}.',
      rf_paid: 'Déjà versés',
      rf_paid_fine: 'Virés sur ton compte jusqu’ici.',

      rf_link_title: 'Ton lien',
      rf_link_lede: 'Toute personne qui s’abonne après l’avoir ouvert devient ton filleul, à vie.',
      rf_copy: 'Copier',
      rf_copied: 'Copié',
      rf_copy_label: 'Copier ton lien',
      rf_copied_toast: 'Ton lien est copié.',
      rf_copy_failed: 'Ce navigateur n’a pas laissé la page copier. Le lien est sélectionné : copie-le au clavier.',
      rf_customize: 'Personnaliser',
      rf_code_label: 'Ton code',
      rf_code_hint: 'De {min} à {max} lettres, chiffres ou tirets. Les liens déjà partagés continuent de marcher.',
      rf_code_bad: 'De {min} à {max} lettres minuscules, chiffres ou tirets simples, sans tiret au début ni à la fin.',
      rf_code_save: 'Enregistrer',
      rf_code_saved: 'Ton lien est maintenant {link}',
      err_code_invalid: 'De 3 à 20 lettres minuscules, chiffres ou tirets simples, sans tiret au début ni à la fin.',
      err_code_forbidden: 'Ce code est réservé. Choisis-en un autre.',
      err_code_taken: 'Ce code est déjà pris. Essaie avec un autre.',
      rf_qr: 'QR code',
      rf_qr_label: 'QR code de {link}',
      rf_qr_download: 'Télécharger en PNG',
      rf_share: 'Partager',
      rf_share_on: 'Partager sur {where}',
      rf_share_more: 'Plus…',
      rf_share_email: 'E-mail',
      rf_discord_toast: 'Message copié : colle-le dans un salon Discord.',
      rf_message: 'Je fais tourner mes agents Claude Code et Codex côte à côte avec HangarAI. Essaie-le avec mon lien : 1 000 crédits offerts à l’abonnement. {link}',
      rf_subject: 'Essaie HangarAI',

      rf_how: 'Comment ça marche',
      rf_step1: 'Partage ton lien',
      rf_step1_text: 'En live, dans une vidéo, un post, un README ou un message à ton équipe.',
      rf_step2: 'Ton filleul reçoit {n} crédits',
      rf_step2_text: 'Des crédits offerts avec son premier paiement, en plus de son offre.',
      rf_step3: 'Tu touches {rate} %, à vie',
      rf_step3_text: 'De ce que HangarAI reçoit de lui, hors TVA et frais de paiement : chaque paiement, tant qu’il paie.',
      rf_how_fine: 'Chaque commission est retenue {days} jours, le temps d’un remboursement, puis versée le 1er du mois dès {min}, par virement ou PayPal.',
      rf_terms: 'Conditions',
      rf_faq: 'Questions',

      rf_sim: 'Combien ça pourrait faire ?',
      rf_sim_plan: 'Leur offre',
      rf_sim_label: 'Filleuls abonnés',
      rf_sim_people: '{n} filleuls',
      rf_sim_people_one: '1 filleul',
      rf_sim_month: 'par mois',
      rf_sim_year: '{amount} par an, tant qu’ils paient',
      rf_sim_fine: 'Avec {plan} au mois : {cut} par mois et par filleul, {rate} % de ce que garde HangarAI après TVA et frais. Une estimation, pas une promesse.',

      rf_funnel: 'De la visite au client',
      rf_funnel_aside: 'Depuis la création de ton lien',
      rf_visits: 'Visites',
      rf_trials: 'Essais',
      rf_customers: 'Clients',
      rf_visits_fine: 'ont ouvert ton lien',
      rf_trials_fine: 'ont lancé l’essai de 14 jours',
      rf_customers_fine: 'ont payé au moins une fois',
      rf_conv: '{pct} des visites',
      rf_conv_trials: '{pct} des essais',
      rf_weeks: '12 dernières semaines',
      rf_week_tip: 'Semaine du {date} : {n}',
      rf_weeks_caption: '{what} par semaine, les 12 dernières semaines',
      rf_week_col: 'Semaine du',
      rf_funnel_empty: 'Rien pour l’instant. Une visite apparaît ici quelques minutes après l’ouverture de ton lien.',

      rf_people: 'Tes filleuls',
      rf_people_aside: '{n} clients · {m} en essai',
      rf_people_aside_one: '1 client · {m} en essai',
      rf_p_pro_monthly: 'Un abonné Pro, au mois',
      rf_p_pro_yearly: 'Un abonné Pro, à l’année',
      rf_p_max_monthly: 'Un abonné Max, au mois',
      rf_p_max_yearly: 'Un abonné Max, à l’année',
      rf_p_lifetime: 'Un détenteur de Lifetime',
      rf_p_credits: 'Un acheteur de crédits',
      rf_p_trial: 'En essai de 14 jours',
      rf_p_ended: 'Un ancien abonné',
      rf_p_refunded: 'Remboursé',
      rf_since: 'depuis {when}',
      rf_started: 'commencé {when}',
      rf_per_month: '+{amount} / mois',
      rf_once: '{amount} une fois',
      rf_so_far: '{amount} jusqu’ici',
      rf_nothing_yet: 'Rien encore',
      rf_people_more: 'Tout afficher ({n})',
      rf_people_less: 'Afficher moins',
      rf_people_beyond: 'Et {n} de plus, non listés.',
      rf_people_empty: 'Ton premier filleul te rapportera ~{amount} par mois, à vie.',
      rf_people_empty_text: 'Il apparaît ici dès qu’il lance un essai avec ton lien, jamais par son nom : son identité reste la sienne.',
      rf_copy_link: 'Copier ton lien',

      rf_badges: 'Paliers',
      rf_badge_1: 'Premier client',
      rf_badge_n: '{n} clients',
      rf_badge_reached: 'Atteint le {date}',
      rf_badge_togo: 'Encore {n}',
      rf_badge_locked: 'À débloquer',
      rf_badges_fine: 'Des repères en chemin. Le taux reste {rate} % dès le premier client.',

      rf_money: 'Versements et relevés',
      rf_payouts: 'Virements',
      rf_payouts_count: '{n} jusqu’ici',
      rf_payouts_empty: 'Pas encore de virement. Le premier part le 1er d’un mois, dès que {min} sont à verser.',
      rf_statements: 'Relevés',
      rf_statements_count: '{n} mois',
      rf_statements_empty: 'Ton premier relevé apparaît le mois où ta première commission est enregistrée.',
      rf_statements_error: 'Les relevés n’ont pas pu être chargés.',
      rf_st_earned: '{amount} gagnés',
      rf_st_cancelled: '{amount} annulés',
      rf_st_paid: '{amount} versés le {date}',
      rf_st_open: 'Voir',
      rf_st_open_label: 'Voir le relevé de {month}',
      rf_st_csv: 'CSV',
      rf_st_csv_label: 'Télécharger le relevé de {month} en CSV',
      rf_st_title: 'Relevé · {month}',
      rf_st_payee: 'Versé à {name} ({country})',
      rf_st_date: 'Date',
      rf_st_ref: 'Filleul',
      rf_st_what: 'Quoi',
      rf_st_net: 'Net',
      rf_st_commission: 'Commission',
      rf_st_status: 'État',
      rf_st_total: 'Gagné ce mois-ci',
      rf_st_total_cancelled: 'Annulé ce mois-ci',
      rf_st_payout: 'Virement du {date} · réf. {ref}',
      rf_st_no_payout: 'Pas de virement ce mois-ci.',
      rf_st_print: 'Imprimer ou enregistrer en PDF',
      rf_st_close: 'Fermer',
      rf_st_issuer: 'Programme de parrainage HangarAI · hangarai.dev',
      rf_s_pending: 'En délai',
      rf_s_payable: 'À verser',
      rf_s_paid: 'Versée',
      rf_s_cancelled: 'Annulée',
      rf_k_payment: 'Paiement {product}',
      rf_k_refund: 'Remboursement {product}',
      rf_prod_pro_monthly: 'Pro, mensuel',
      rf_prod_pro_yearly: 'Pro, annuel',
      rf_prod_max_monthly: 'Max, mensuel',
      rf_prod_max_yearly: 'Max, annuel',
      rf_prod_lifetime: 'Lifetime',
      rf_prod_credits: 'Crédits',
      rf_payouts_next: 'Prochain versement',

      rf_payout: 'Coordonnées de versement',
      rf_payout_lede: 'Où partent tes commissions, une fois par mois.',
      rf_payout_lock: 'Chiffrées sur notre serveur et utilisées seulement pour te payer. Le numéro de compte n’est plus jamais affiché en entier.',
      rf_payout_none: 'Ajoute-les une fois, et chaque versement part tout seul.',
      rf_payout_edit: 'Modifier',
      rf_legal_name: 'Nom légal',
      rf_legal_hint: 'Le nom du compte qui reçoit l’argent : le tien, ou celui de ta société.',
      rf_country: 'Pays',
      rf_country_pick: 'Choisir…',
      rf_method: 'Versé par',
      rf_m_iban: 'Virement bancaire',
      rf_m_paypal: 'PayPal',
      rf_iban: 'IBAN',
      rf_iban_ok: 'Valide',
      rf_bic: 'BIC',
      rf_optional: 'facultatif',
      rf_paypal_email: 'E-mail PayPal',
      rf_accept: 'J’accepte les {terms}.',
      rf_accept_terms: 'conditions du programme d’affiliation',
      rf_payout_save: 'Enregistrer mes coordonnées',
      rf_payout_saved: 'Coordonnées de versement enregistrées.',
      rf_tax: 'Les commissions sont un revenu : c’est à toi de les déclarer là où tu vis.',
      rf_f_name: 'Saisis le nom du bénéficiaire.',
      rf_f_country: 'Choisis un pays.',
      err_iban_invalid: 'Cet IBAN n’est pas valide. Compare-le à celui qu’affiche ta banque.',
      err_paypal_invalid: 'Ce n’est pas une adresse e-mail.',
      err_terms_needed: 'Accepte les conditions pour recevoir tes versements.',
      rf_updated: 'Mises à jour le {date}'
    }
  };

  // The version of site/legal/affiliate-terms.html (the date on it).
  var TERMS = '2026-11-19';

  /* ── 2. countries and IBANs ────────────────────────────────── */

  var COUNTRIES = ('AD AE AF AG AI AL AM AO AR AS AT AU AW AX AZ BA BB BD BE BF BG BH BI BJ BL BM BN BO BQ BR BS BT ' +
    'BW BY BZ CA CC CD CF CG CH CI CK CL CM CN CO CR CU CV CW CX CY CZ DE DJ DK DM DO DZ EC EE EG EH ER ES ET FI FJ FK ' +
    'FM FO FR GA GB GD GE GF GG GH GI GL GM GN GP GQ GR GT GU GW GY HK HN HR HT HU ID IE IL IM IN IQ IR IS IT JE JM JO ' +
    'JP KE KG KH KI KM KN KP KR KW KY KZ LA LB LC LI LK LR LS LT LU LV LY MA MC MD ME MF MG MH MK ML MM MN MO MP MQ MR ' +
    'MS MT MU MV MW MX MY MZ NA NC NE NF NG NI NL NO NP NR NU NZ OM PA PE PF PG PH PK PL PM PN PR PS PT PW PY QA RE RO ' +
    'RS RU RW SA SB SC SD SE SG SH SI SJ SK SL SM SN SO SR SS ST SV SX SY SZ TC TD TG TH TJ TK TL TM TN TO TR TT TV TW ' +
    'TZ UA UG US UY UZ VA VC VE VG VI VN VU WF WS XK YE YT ZA ZM ZW').split(' ');

  // ISO 13616: the length of each country's IBAN that uses one in SEPA
  // and around; a country not listed is checked by its digits only.
  var IBAN_LENGTHS = {
    AD: 24, AE: 23, AT: 20, BA: 20, BE: 16, BG: 22, BH: 22, BR: 29, CH: 21, CR: 22, CY: 28, CZ: 24, DE: 22, DK: 18,
    DO: 28, EE: 20, EG: 29, ES: 24, FI: 18, FO: 18, FR: 27, GB: 22, GE: 22, GI: 23, GL: 18, GR: 27, GT: 28, HR: 21,
    HU: 28, IE: 22, IL: 23, IS: 26, IT: 27, JO: 30, KW: 30, KZ: 20, LB: 28, LI: 21, LT: 20, LU: 20, LV: 21, MC: 27,
    MD: 24, ME: 22, MK: 19, MR: 27, MT: 31, MU: 30, NL: 18, NO: 15, PK: 24, PL: 28, PS: 29, PT: 25, QA: 29, RO: 24,
    RS: 22, SA: 24, SE: 24, SI: 19, SK: 24, SM: 27, TN: 24, TR: 26, UA: 29, VA: 22, VG: 24, XK: 20
  };

  function ibanClean(raw) {
    return String(raw || '').toUpperCase().replace(/[^A-Z0-9]/g, '');
  }

  // Its country's length, then the check digits: mod 97 of the number
  // with its first four characters moved to the end must be 1.
  function ibanValid(iban) {
    if (!/^[A-Z]{2}\d{2}[A-Z0-9]{11,30}$/.test(iban)) return false;
    var len = IBAN_LENGTHS[iban.slice(0, 2)];
    if (len && len !== iban.length) return false;
    var moved = iban.slice(4) + iban.slice(0, 4);
    var rest = 0;
    for (var i = 0; i < moved.length; i++) {
      var c = moved.charCodeAt(i);
      var v = c >= 65 ? String(c - 55) : moved[i];
      for (var j = 0; j < v.length; j++) rest = (rest * 10 + (+v[j])) % 97;
    }
    return rest === 1;
  }

  function ibanGroups(iban) {
    return iban.replace(/(.{4})/g, '$1 ').trim();
  }

  var CODE_RULE = /^[a-z0-9](?:[a-z0-9]|-(?!-))*[a-z0-9]$/;

  /* ── 3. the view ───────────────────────────────────────────── */

  function create(ctx) {
    var h = ctx.h;
    var t = ctx.t;
    var state = ctx.state;
    var reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    var NS = 'http://www.w3.org/2000/svg';

    var R = {
      data: null,           // GET /v1/referral
      statements: null,     // GET /v1/referral/statements, its list
      stError: false,
      loading: false,
      error: false,
      played: false,        // the motion played once
      sim: 10,
      simPlan: 'pro',       // the simulator's plan: pro or max
      codeEdit: false,
      codeValue: null,
      codeError: null,
      allPeople: false,
      payoutEdit: false,
      form: null,
      formErrors: {},
      formError: null,
      open: {}              // which accordions are open
    };
    state.referral = R;

    function api() {
      return ctx.api();
    }

    function load(opts) {
      opts = opts || {};
      if (R.loading || (R.data && !opts.fresh)) return Promise.resolve();
      R.loading = true;
      R.error = false;
      if (!opts.quiet) ctx.render();
      var stats = api().statements().then(function (res) {
        R.statements = res.statements || [];
        R.stError = false;
      }, function (err) {
        if (err.status === 401) throw err;
        R.stError = true;
      });
      return Promise.all([api().referral(), stats]).then(function (both) {
        R.data = both[0];
        R.loading = false;
        ctx.render();
      }, function (err) {
        R.loading = false;
        if (err.status === 401) return ctx.reload();
        R.error = true;
        ctx.render();
      });
    }

    /* formats ------------------------------------------------------- */

    function lang() {
      return ctx.lang();
    }

    // Money as earnings read best: always its cents (€0.00, €13.46),
    // or whole when `short` and it is round (€20).
    function money(p, short) {
      if (!p) return '';
      var whole = short && p.amount % 100 === 0;
      return ctx.nbsp(new Intl.NumberFormat(lang(), {
        style: 'currency', currency: p.currency || 'EUR',
        minimumFractionDigits: whole ? 0 : 2, maximumFractionDigits: whole ? 0 : 2
      }).format(p.amount / 100));
    }

    function eur(cents, currency) {
      return { amount: Math.round(cents), currency: currency || 'EUR' };
    }

    function pct(part, whole) {
      if (!whole) return null;
      var v = part / whole * 100;
      return ctx.nbsp(new Intl.NumberFormat(lang(), { maximumFractionDigits: v < 10 ? 1 : 0 }).format(v)) + ' %';
    }

    function isoDay(day) {
      return Date.parse(day + 'T12:00:00Z');
    }

    function monthName(key) {
      var parts = key.split('-');
      return new Intl.DateTimeFormat(lang(), { month: 'long', year: 'numeric', timeZone: 'UTC' })
        .format(new Date(Date.UTC(+parts[0], +parts[1] - 1, 15)));
    }

    function sinceMonth(ms) {
      var d = new Date(ms);
      var same = d.getFullYear() === new Date().getFullYear();
      return new Intl.DateTimeFormat(lang(), same ? { month: 'long' } : { month: 'long', year: 'numeric' }).format(d);
    }

    function countryName(code) {
      try {
        return new Intl.DisplayNames([lang()], { type: 'region' }).of(code) || code;
      } catch (e) {
        return code;
      }
    }

    function linkText(link) {
      return String(link || '').replace(/^https?:\/\//, '');
    }

    function example(product) {
      var ex = ((R.data && R.data.program && R.data.program.examples) || []).filter(function (e) { return e.product === product; })[0];
      return ex ? ex.commission : null;
    }

    function program() {
      var p = (R.data && R.data.program) || {};
      return {
        rate: p.rate || 30,
        days: p.hold_days || 30,
        bonus: p.bonus_credits || 1000,
        min: p.payout_minimum || eur(2000)
      };
    }

    /* small drawing helpers -------------------------------------------- */

    function s(tag, attrs, kids) {
      var el = document.createElementNS(NS, tag);
      Object.keys(attrs || {}).forEach(function (k) { el.setAttribute(k, attrs[k]); });
      (kids || []).forEach(function (kid) { if (kid) el.appendChild(kid); });
      return el;
    }

    var ICON = {
      copy: '<rect x="5.25" y="5.25" width="8" height="8.5" rx="1.6" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M10.75 5.25V3.6a1.35 1.35 0 0 0-1.35-1.35H4.1A1.35 1.35 0 0 0 2.75 3.6v6.3c0 .75.6 1.35 1.35 1.35h1.15" fill="none" stroke="currentColor" stroke-width="1.3"/>',
      check: '<path d="M3.5 8.4 6.6 11.4 12.6 4.8" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>',
      pencil: '<path d="M10.6 2.9 13.1 5.4 6 12.5l-3.2.7.7-3.2 7.1-7.1Z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>',
      lock: '<rect x="3" y="7" width="10" height="7" rx="1.6" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M5.25 7V5.1a2.75 2.75 0 0 1 5.5 0V7" fill="none" stroke="currentColor" stroke-width="1.3"/><circle cx="8" cy="10.5" r="1" fill="currentColor"/>',
      download: '<path d="M8 2.5v7.75M4.75 7.25 8 10.5l3.25-3.25M3 13.5h10" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>',
      x: '<path d="M3 2.75h3.1l6.9 10.5H9.9L3 2.75Z" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"/><path d="M12.9 2.75 8.9 7.3M7.1 8.7l-4 4.55" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"/>',
      discord: '<path d="M5.4 3.6c-1 .2-1.9.5-2.7 1C1.5 6.6 1.1 8.6 1.3 10.6c1 .8 2 1.3 3 1.6l.7-1.1M10.6 3.6c1 .2 1.9.5 2.7 1 1.2 2 1.6 4 1.4 6-1 .8-2 1.3-3 1.6l-.7-1.1M4.4 10.7c2.4 1 4.8 1 7.2 0M4.6 4.4c2.3-.7 4.5-.7 6.8 0" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/><circle cx="6" cy="8.1" r="1" fill="currentColor"/><circle cx="10" cy="8.1" r="1" fill="currentColor"/>',
      reddit: '<ellipse cx="8" cy="9.6" rx="5.6" ry="3.9" fill="none" stroke="currentColor" stroke-width="1.2"/><circle cx="2.9" cy="7.2" r="1.15" fill="none" stroke="currentColor" stroke-width="1.1"/><circle cx="13.1" cy="7.2" r="1.15" fill="none" stroke="currentColor" stroke-width="1.1"/><path d="M8 5.7 9 2.3l2.6.6" fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round"/><circle cx="12.4" cy="3.1" r=".95" fill="currentColor"/><circle cx="6" cy="9.1" r=".85" fill="currentColor"/><circle cx="10" cy="9.1" r=".85" fill="currentColor"/><path d="M6.1 11.1c1.2.7 2.6.7 3.8 0" fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round"/>',
      linkedin: '<rect x="2" y="2" width="12" height="12" rx="2.2" fill="none" stroke="currentColor" stroke-width="1.25"/><path d="M5.2 7.1v4.4M8 11.5V7.1m0 1.9c0-1 .8-1.9 1.8-1.9s1.6.7 1.6 1.8v2.6" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/><circle cx="5.2" cy="5" r=".9" fill="currentColor"/>',
      mail: '<rect x="1.75" y="3.25" width="12.5" height="9.5" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="m2.5 4.25 5.5 4.25 5.5-4.25" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>',
      share: '<path d="M8 2.25v8M5 5.1 8 2.25l3 2.85M4.5 7.25h-.75a1 1 0 0 0-1 1v4.5a1 1 0 0 0 1 1h8.5a1 1 0 0 0 1-1v-4.5a1 1 0 0 0-1-1h-.75" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>',
      link: '<path d="M6.9 9.1a2.9 2.9 0 0 0 4.1 0l2-2a2.9 2.9 0 0 0-4.1-4.1l-.9.9M9.1 6.9a2.9 2.9 0 0 0-4.1 0l-2 2a2.9 2.9 0 0 0 4.1 4.1l.9-.9" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>',
      gift: '<rect x="2.25" y="6" width="11.5" height="3" rx=".8" fill="none" stroke="currentColor" stroke-width="1.25"/><path d="M3.25 9v4a1 1 0 0 0 1 1h7.5a1 1 0 0 0 1-1V9M8 6v8M8 6S6.9 2.3 5.1 3.1C3.7 3.8 5.3 6 8 6Zm0 0s1.1-3.7 2.9-2.9C12.3 3.8 10.7 6 8 6Z" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"/>',
      percent: '<path d="M12.5 3.5 3.5 12.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/><circle cx="4.6" cy="4.6" r="1.75" fill="none" stroke="currentColor" stroke-width="1.3"/><circle cx="11.4" cy="11.4" r="1.75" fill="none" stroke="currentColor" stroke-width="1.3"/>',
      person: '<circle cx="8" cy="5.5" r="2.6" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M2.9 13.6c.7-2.4 2.7-3.8 5.1-3.8s4.4 1.4 5.1 3.8" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>',
      infinity: '<path d="M8 8c-1.3-1.7-2.4-2.6-3.6-2.6a2.6 2.6 0 0 0 0 5.2C5.6 10.6 6.7 9.7 8 8Zm0 0c1.3 1.7 2.4 2.6 3.6 2.6a2.6 2.6 0 0 0 0-5.2C10.4 5.4 9.3 6.3 8 8Z" fill="none" stroke="currentColor" stroke-width="1.3"/>',
      clock: '<circle cx="8" cy="8" r="5.75" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M8 4.9V8l2.1 1.4" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/>',
      chevron: '<path d="m6 3.75 4.25 4.25L6 12.25" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>',
      doc: '<path d="M4 1.75h5.4L12.5 4.9v8.35a1 1 0 0 1-1 1h-7.5a1 1 0 0 1-1-1V2.75a1 1 0 0 1 1-1Z" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linejoin="round"/><path d="M9.25 1.9V5h3.1M5.5 8.25h5M5.5 10.75h3.5" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"/>'
    };

    function icon(name, size) {
      var el = document.createElementNS(NS, 'svg');
      el.setAttribute('width', size || 16);
      el.setAttribute('height', size || 16);
      el.setAttribute('viewBox', '0 0 16 16');
      el.setAttribute('aria-hidden', 'true');
      el.innerHTML = ICON[name]; // constant markup of this file, never data
      return el;
    }

    function btn(label, opts) {
      opts = opts || {};
      var cls = 'btn' + (opts.primary ? ' btn--primary' : '') + (opts.quiet ? ' btn--quiet' : '') +
        (opts.small ? ' btn--sm' : '') + (opts.cls ? ' ' + opts.cls : '');
      return h('button', {
        type: opts.submit ? 'submit' : 'button', 'class': cls, id: opts.id || null,
        'aria-label': opts.label || null, 'aria-busy': opts.busy ? 'true' : null,
        onclick: opts.busy ? null : opts.onclick
      }, opts.icon ? icon(opts.icon) : null, label ? h('span', label) : null);
    }

    function copyText(text) {
      if (navigator.clipboard && window.isSecureContext) {
        return navigator.clipboard.writeText(text).catch(function () { return legacyCopy(text); });
      }
      return legacyCopy(text);
    }

    function legacyCopy(text) {
      return new Promise(function (resolve, reject) {
        var area = h('textarea', { 'aria-hidden': 'true', readonly: true, style: 'position:fixed;top:-100px;opacity:0' });
        area.value = text;
        document.body.appendChild(area);
        area.select();
        var ok = false;
        try { ok = document.execCommand('copy'); } catch (e) { ok = false; }
        area.remove();
        if (ok) resolve(); else reject(new Error('copy'));
      });
    }

    function save(name, blob) {
      var url = URL.createObjectURL(blob);
      var a = h('a', { href: url, download: name, style: 'display:none' });
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(function () { URL.revokeObjectURL(url); }, 4000);
    }

    /* the view ------------------------------------------------------- */

    function view() {
      if (!R.data) {
        if (R.error) {
          return h('div.rf',
            h('section.panel.rf-fail',
              h('div.notice.notice--error', { role: 'alert' }, h('p', t('rf_error'))),
              h('p.rf-fail__retry', ctx.button(t('retry'), { small: true, busy: R.loading, onclick: function () { load(); } }))
            ));
        }
        return bones();
      }
      var d = R.data;
      // Wide, three columns: the link, the simulator and the funnel,
      // then the referrals, the milestones and the payouts. Narrower,
      // two (the funnel under the first two, the milestones over the
      // payouts beside the referrals); on a phone, one.
      var root = h('div.rf' + (R.played || reduced ? '' : '.rf--intro'),
        hero(d),
        h('div.rf-row.rf-row--a', linkPanel(d), simPanel(d), funnelPanel(d)),
        howPanel(d),
        h('div.rf-row.rf-row--b', peoplePanel(d), h('div.rf-col', badgesPanel(d), moneyPanel(d))),
        payoutPanel(d)
      );
      return root;
    }

    function bones() {
      function bone(cls) { return h('div.bone' + (cls ? '.' + cls : '')); }
      return h('div.rf.rf--loading', { 'aria-busy': 'true' },
        h('p.sr-only', { role: 'status' }, t('rf_loading')),
        h('div.panel.panel--bones.rf-hero--bones', { 'aria-hidden': 'true' }, bone('bone--line'), bone('bone--hero'), bone('bone--line'), bone('bone--meter')),
        h('div.rf-row.rf-row--a', { 'aria-hidden': 'true' },
          h('div.panel.panel--bones', bone('bone--line'), bone('bone--row'), bone('bone--row')),
          h('div.panel.panel--bones', bone('bone--line'), bone('bone--big'), bone('bone--line')),
          h('div.panel.panel--bones.rf-funnel', bone('bone--line'), bone('bone--row'), bone('bone--row')))
      );
    }

    /* the hero: what was earned, and what comes next ------------------ */

    function hero(d) {
      var e = d.earnings;
      var p = program();
      var first = e.earned.amount <= 0 && d.stats.customers === 0;
      var paying = (d.referrals || []).filter(function (r) { return r.status === 'active' && r.monthly; }).length;
      var monthly = e.monthly && e.monthly.amount > 0;

      var main;
      if (first) {
        var cut = example('pro_monthly') || eur(447);
        main = h('div.rf-hero__main',
          h('p.rf-hero__label', t('rf_first_label')),
          h('p.rf-hero__fig',
            h('span.rf-hero__num.rf-hero__num--soft', { 'data-count': cut.amount, 'data-currency': cut.currency, 'data-prefix': '~' }, '~' + money(cut)),
            h('span.rf-hero__per', t('rf_per_month_life'))),
          h('p.rf-hero__lede', t('rf_first_lede', { n: ctx.num(p.bonus), rate: p.rate }))
        );
      } else {
        main = h('div.rf-hero__main',
          h('p.rf-hero__label', t('rf_earned')),
          h('p.rf-hero__fig',
            h('span.rf-hero__num', { 'data-count': e.earned.amount, 'data-currency': e.earned.currency }, money(e.earned))),
          monthly ? h('p.rf-hero__monthly',
            h('span.rf-hero__plus', t('rf_monthly', { amount: money(e.monthly) })),
            h('span.rf-hero__fine', t(paying === 1 ? 'rf_monthly_fine_one' : 'rf_monthly_fine', { n: ctx.num(paying) }))) : null,
          nextBlock(d)
        );
      }

      return h('section.panel.rf-hero', { 'aria-labelledby': 'rf-title' },
        h('div.rf-hero__glow', { 'aria-hidden': 'true' }),
        h('h2.sr-only#rf-title', { tabindex: '-1' }, t('tab_referral')),
        main,
        first ? preview(d, p) : stack(d)
      );
    }

    // Under the figure: the next transfer, and a meter toward it.
    function nextBlock(d) {
      var e = d.earnings;
      var n = d.next_payout || { state: 'none', amount: eur(0) };
      var p = program();
      var date = ctx.date(isoDay(n.date || new Date().toISOString().slice(0, 10)));
      var line;
      var action = null;
      var ratio = Math.min(1, n.amount.amount / p.min.amount);
      if (n.state === 'scheduled') {
        line = t('rf_next_scheduled', { amount: money(n.amount), date: date });
      } else if (n.state === 'needs_profile') {
        line = t('rf_next_profile', { amount: money(n.amount), date: date });
        action = btn(t('rf_add_details'), { small: true, primary: true, onclick: function () { openPayout(); } });
      } else if (n.state === 'below_minimum') {
        line = t('rf_next_below', { amount: money(n.amount), left: money(eur(p.min.amount - n.amount.amount, p.min.currency)) });
      } else if (e.pending.amount > 0) {
        line = t('rf_next_hold', { amount: money(e.pending), days: p.days });
      } else {
        line = t('rf_next_none', { min: money(p.min, true) });
      }
      var ready = n.state === 'scheduled' || n.state === 'needs_profile';
      return h('div.rf-next',
        h('p.rf-next__line', line),
        h('div.rf-meter' + (ready ? '.is-ready' : ''),
          h('div.rf-meter__label',
            h('span', t('rf_meter_label')),
            h('span.rf-meter__value', n.state === 'scheduled' ? t('rf_meter_ready')
              : n.state === 'needs_profile' ? t('rf_meter_waiting')
                : t('rf_meter_value', { amount: money(n.amount), min: money(p.min, true) }))),
          h('div.rf-meter__track', { role: 'progressbar', 'aria-valuemin': '0', 'aria-valuemax': String(p.min.amount / 100), 'aria-valuenow': String(Math.min(n.amount.amount, p.min.amount) / 100), 'aria-label': t('rf_meter_label') },
            h('i', { style: 'transform:scaleX(' + ratio.toFixed(4) + ')', 'data-ratio': ratio.toFixed(4) }))),
        e.next_release ? h('p.rf-next__fine', t('rf_release', { amount: money(e.next_release.amount), date: ctx.date(e.next_release.at) })) : null,
        action
      );
    }

    // Beside the figure: pending, payable, paid.
    function stack(d) {
      var e = d.earnings;
      var p = program();
      function row(key, value, fine, tone) {
        return h('div.rf-stat.rf-stat--' + tone,
          h('dt', h('i', { 'aria-hidden': 'true' }), t(key)),
          h('dd', h('span.rf-stat__num', money(value)), h('span.rf-stat__fine', fine)));
      }
      return h('dl.rf-stack',
        row('rf_pending', e.pending, t('rf_pending_fine', { days: p.days }), 'hold'),
        row('rf_payable', e.payable, t('rf_payable_fine', { min: money(p.min, true) }), 'payable'),
        row('rf_paid', e.paid, t('rf_paid_fine'), 'paid'));
    }

    // Beside the first-time figure: what a friend sees when they open
    // the link (site/r/), drawn small.
    function preview(d, p) {
      var a = (state.summary && state.summary.account) || {};
      var name = (a.display_name || '').split(/\s+/)[0] || null;
      var initial = (name || '?').slice(0, 1).toUpperCase();
      return h('figure.rf-preview', { 'aria-label': t('rf_preview') },
        h('figcaption.rf-preview__cap', t('rf_preview')),
        h('div.rf-preview__card', { 'aria-hidden': 'true' },
          h('div.rf-preview__bar', h('i'), h('i'), h('i'), h('span', linkText(d.link))),
          h('div.rf-preview__body',
            h('span.rf-preview__avatar', initial),
            h('p.rf-preview__who', name ? t('rf_preview_who', { name: name }) : t('rf_preview_friend')),
            h('p.rf-preview__gift', icon('gift', 14), t('rf_preview_gift', { n: ctx.num(p.bonus) })),
            h('span.rf-preview__cta', t('rf_preview_cta')))));
    }

    /* the link -------------------------------------------------------- */

    function message(d) {
      return t('rf_message', { link: d.link });
    }

    function linkPanel(d) {
      var rules = d.code_rules || { min: 3, max: 20 };
      var head = h('div.panel__head',
        h('h2.panel__title#rf-link-title', { tabindex: '-1' }, t('rf_link_title')),
        !R.codeEdit ? h('button.rf-textbtn', { type: 'button', onclick: function () {
          R.codeEdit = true;
          R.codeValue = d.code;
          R.codeError = null;
          ctx.focus('#rf-code');
          ctx.render();
        } }, icon('pencil', 14), t('rf_customize')) : null);

      var box;
      if (R.codeEdit) {
        box = h('form.rf-code', {
          novalidate: true,
          onsubmit: function (ev) { ev.preventDefault(); saveCode(rules); }
        },
          h('label.rf-code__label', { 'for': 'rf-code' }, t('rf_code_label')),
          h('div.rf-code__row',
            h('div.rf-code__field' + (R.codeError ? '.is-bad' : ''),
              h('span.rf-code__prefix', { 'aria-hidden': 'true' }, linkText(d.link).replace(/[^/]*$/, '')),
              h('input#rf-code', {
                type: 'text', value: R.codeValue || '', maxlength: String(rules.max), autocomplete: 'off',
                autocapitalize: 'none', spellcheck: 'false', 'aria-describedby': 'rf-code-hint',
                'aria-invalid': R.codeError ? 'true' : null,
                oninput: function (ev) {
                  var v = ev.target.value.toLowerCase().replace(/[^a-z0-9-]/g, '');
                  if (v !== ev.target.value) ev.target.value = v;
                  R.codeValue = v;
                  var hint = document.getElementById('rf-code-hint');
                  var bad = v.length >= rules.min && !CODE_RULE.test(v);
                  if (hint) {
                    hint.textContent = bad ? t('rf_code_bad', rules) : t('rf_code_hint', rules);
                    hint.classList.toggle('is-bad', bad);
                  }
                }
              })),
            btn(t('rf_code_save'), { small: true, primary: true, submit: true, busy: R.codeBusy }),
            btn(t('cancel'), { small: true, quiet: true, onclick: function () {
              R.codeEdit = false;
              R.codeError = null;
              ctx.focus('#rf-copy');
              ctx.render();
            } })),
          h('p.rf-code__hint#rf-code-hint' + (R.codeError ? '.is-bad' : ''), { role: R.codeError ? 'alert' : null },
            R.codeError ? t(R.codeError) : t('rf_code_hint', rules))
        );
      } else {
        box = h('div.rf-linkbox',
          h('span.rf-linkbox__icon', icon('link')),
          h('span.rf-linkbox__text', { translate: 'no' },
            h('span.rf-linkbox__host', linkText(d.link).replace(/[^/]*$/, '')),
            h('b', d.code)),
          h('button.btn.btn--primary.btn--sm.rf-copy#rf-copy', {
            type: 'button', 'aria-label': t('rf_copy_label'),
            onclick: function (ev) { copyLink(d, ev.currentTarget); }
          }, h('span.rf-copy__icons', icon('copy'), icon('check')), h('span.rf-copy__text', h('span', t('rf_copy')), h('span', t('rf_copied')))));
      }

      return h('section.panel.rf-link', { 'aria-labelledby': 'rf-link-title' },
        head,
        h('p.panel__aside.rf-link__lede', t('rf_link_lede')),
        box,
        h('div.rf-link__body',
          qrBlock(d),
          shareBlock(d))
      );
    }

    function copyLink(d, el) {
      copyText(d.link).then(function () {
        el.classList.remove('is-copied');
        void el.offsetWidth;
        el.classList.add('is-copied');
        clearTimeout(el._copied);
        el._copied = setTimeout(function () { el.classList.remove('is-copied'); }, 1800);
        ctx.announce(t('rf_copied_toast'));
      }, function () {
        // The browser refused: select the link, for the keyboard to copy.
        var text = document.querySelector('.rf-linkbox__text');
        if (text && window.getSelection) {
          var range = document.createRange();
          range.selectNodeContents(text);
          var sel = window.getSelection();
          sel.removeAllRanges();
          sel.addRange(range);
        }
        ctx.toast(t('rf_copy_failed'));
      });
    }

    function saveCode(rules) {
      var code = (R.codeValue || '').trim();
      if (code === R.data.code) {
        R.codeEdit = false;
        return ctx.render();
      }
      if (code.length < rules.min || code.length > rules.max || !CODE_RULE.test(code)) {
        R.codeError = 'err_code_invalid';
        ctx.focus('#rf-code');
        return ctx.render();
      }
      R.codeBusy = true;
      R.codeError = null;
      ctx.render();
      api().setReferralCode(code).then(function (res) {
        R.codeBusy = false;
        R.codeEdit = false;
        R.data = res;
        ctx.toast(t('rf_code_saved', { link: linkText(res.link) }));
        ctx.focus('#rf-copy');
        ctx.render();
      }, function (err) {
        R.codeBusy = false;
        if (err.status === 401) return ctx.reload();
        R.codeError = ctx.errorKey(err);
        ctx.focus('#rf-code');
        ctx.render();
      });
    }

    var qrCache = { text: null, qr: null };

    function qrOf(text) {
      if (qrCache.text !== text) {
        qrCache.text = text;
        try { qrCache.qr = window.HangarQR ? window.HangarQR.make(text, 'M') : null; } catch (e) { qrCache.qr = null; }
      }
      return qrCache.qr;
    }

    function qrBlock(d) {
      var qr = qrOf(d.link);
      if (!qr) return null;
      var dim = qr.size + 8;
      var art = s('svg', { viewBox: '0 0 ' + dim + ' ' + dim, width: '132', height: '132', 'shape-rendering': 'crispEdges', role: 'img', 'aria-label': t('rf_qr_label', { link: linkText(d.link) }) },
        [s('rect', { width: String(dim), height: String(dim), fill: '#fff' }), s('path', { d: window.HangarQR.path(qr, 4), fill: '#0c0c0e' })]);
      return h('figure.rf-qr',
        h('div.rf-qr__tile', art),
        h('figcaption',
          h('button.rf-textbtn', { type: 'button', onclick: function () { downloadQr(d, qr); } }, icon('download', 14), t('rf_qr_download'))));
    }

    // A PNG of 1 024 px, black on white with its quiet zone, for a
    // stream's overlay or a slide.
    function downloadQr(d, qr) {
      var dim = qr.size + 8;
      var scale = Math.floor(1024 / dim);
      var canvas = document.createElement('canvas');
      canvas.width = canvas.height = dim * scale;
      var g = canvas.getContext('2d');
      g.fillStyle = '#fff';
      g.fillRect(0, 0, canvas.width, canvas.height);
      g.fillStyle = '#000';
      for (var y = 0; y < qr.size; y++) {
        for (var x = 0; x < qr.size; x++) if (qr.dark(x, y)) g.fillRect((x + 4) * scale, (y + 4) * scale, scale, scale);
      }
      canvas.toBlob(function (blob) {
        if (blob) save('hangarai-' + d.code + '-qr.png', blob);
      }, 'image/png');
    }

    function shareBlock(d) {
      var text = message(d);
      var subject = t('rf_subject');
      function out(name, where, href) {
        return h('a.rf-share__btn', {
          href: href, target: '_blank', rel: 'noopener noreferrer',
          'aria-label': t('rf_share_on', { where: where })
        }, icon(name), h('span', where));
      }
      var list = [
        out('x', 'X', 'https://x.com/intent/post?text=' + encodeURIComponent(text)),
        h('button.rf-share__btn', {
          type: 'button', 'aria-label': t('rf_share_on', { where: 'Discord' }),
          onclick: function () {
            copyText(text).then(function () { ctx.toast(t('rf_discord_toast')); }, function () { ctx.toast(t('err_generic')); });
          }
        }, icon('discord'), h('span', 'Discord')),
        out('reddit', 'Reddit', 'https://www.reddit.com/submit?url=' + encodeURIComponent(d.link) + '&title=' + encodeURIComponent(subject + ': ' + t('rf_step2', { n: ctx.num(program().bonus) }))),
        out('linkedin', 'LinkedIn', 'https://www.linkedin.com/sharing/share-offsite/?url=' + encodeURIComponent(d.link)),
        h('a.rf-share__btn', { href: 'mailto:?subject=' + encodeURIComponent(subject) + '&body=' + encodeURIComponent(text) }, icon('mail'), h('span', t('rf_share_email')))
      ];
      if (navigator.share) {
        list.push(h('button.rf-share__btn', {
          type: 'button',
          onclick: function () {
            navigator.share({ title: subject, text: text.replace(d.link, '').trim(), url: d.link }).catch(function () { /* dismissed */ });
          }
        }, icon('share'), h('span', t('rf_share_more'))));
      }
      return h('div.rf-share',
        h('p.rf-share__title', t('rf_share')),
        h('div.rf-share__list', list),
        h('p.rf-share__msg', h('span.sr-only', t('rf_share') + ': '), text));
    }

    /* the simulator ---------------------------------------------------- */

    // What one friend on the plan brings each month, by the month: the
    // server's example (credits-economics.md, "The referral program at
    // 30 %": Pro 4.47 €, Max 8.85 €), these when it has none.
    var SIM_CUT = { pro: 447, max: 885 };

    function simPanel() {
      var p = program();
      var plan = R.simPlan === 'max' ? 'max' : 'pro';
      var cut = example(plan + '_monthly') || eur(SIM_CUT[plan]);
      var STOPS = [10, 50, 100];
      var n = R.sim;
      function monthOf(k) { return eur(cut.amount * k, cut.currency); }
      function people(k) { return k === 1 ? t('rf_sim_people_one') : t('rf_sim_people', { n: ctx.num(k) }); }
      function update(k) {
        R.sim = k;
        var root = document.getElementById('rf-sim');
        if (!root) return;
        root.querySelector('.rf-sim__n').textContent = people(k);
        root.querySelector('.rf-sim__amount').textContent = money(monthOf(k));
        root.querySelector('.rf-sim__year').textContent = t('rf_sim_year', { amount: money(eur(cut.amount * k * 12, cut.currency)) });
        var range = root.querySelector('input');
        range.setAttribute('aria-valuetext', people(k) + ', ' + money(monthOf(k)) + ' ' + t('rf_sim_month'));
        range.style.setProperty('--fill', ((k - 1) / 199 * 100).toFixed(2) + '%');
        Array.prototype.forEach.call(root.querySelectorAll('.chip-btn'), function (b) {
          b.setAttribute('aria-pressed', String(+b.dataset.n === k));
        });
      }
      var range = h('input.rf-range#rf-sim-range', {
        type: 'range', min: '1', max: '200', step: '1', value: String(n),
        'aria-labelledby': 'rf-sim-label',
        'aria-valuetext': people(n) + ', ' + money(monthOf(n)) + ' ' + t('rf_sim_month'),
        style: '--fill:' + ((n - 1) / 199 * 100).toFixed(2) + '%',
        oninput: function (ev) { update(+ev.target.value); }
      });
      var plans = h('div.rf-seg.rf-sim__seg', { role: 'radiogroup', 'aria-labelledby': 'rf-sim-plan-label' },
        ['pro', 'max'].map(function (k) {
          return h('button.rf-seg__btn', {
            type: 'button', role: 'radio', 'aria-checked': String(plan === k), tabindex: plan === k ? '0' : '-1',
            id: 'rf-sim-plan-' + k,
            onclick: function () {
              if (R.simPlan === k) return;
              R.simPlan = k;
              ctx.focus('#rf-sim-plan-' + k);
              ctx.render();
            },
            onkeydown: function (ev) {
              if (ev.key !== 'ArrowLeft' && ev.key !== 'ArrowRight' && ev.key !== 'ArrowUp' && ev.key !== 'ArrowDown') return;
              ev.preventDefault();
              R.simPlan = plan === 'pro' ? 'max' : 'pro';
              ctx.focus('#rf-sim-plan-' + R.simPlan);
              ctx.render();
            }
          }, k === 'max' ? 'Max' : 'Pro');
        }));
      return h('section.panel.rf-sim#rf-sim', { 'aria-labelledby': 'rf-sim-title' },
        h('div.panel__head', h('h2.panel__title#rf-sim-title', t('rf_sim'))),
        h('div.rf-sim__plan', h('span.rf-sim__label#rf-sim-plan-label', t('rf_sim_plan')), plans),
        h('p.rf-sim__label#rf-sim-label', t('rf_sim_label')),
        h('p.rf-sim__n', { 'aria-hidden': 'true' }, people(n)),
        range,
        h('div.chips.rf-sim__stops',
          STOPS.map(function (k) {
            return h('button.chip-btn', {
              type: 'button', 'data-n': String(k), 'aria-pressed': String(k === n),
              onclick: function () {
                var input = document.getElementById('rf-sim-range');
                if (input) input.value = String(k);
                update(k);
              }
            }, ctx.num(k));
          })),
        h('div.rf-sim__out', { 'aria-live': 'polite' },
          h('p.rf-sim__fig', h('span.rf-sim__amount', money(monthOf(n))), h('span.rf-sim__per', t('rf_sim_month'))),
          h('p.rf-sim__year', t('rf_sim_year', { amount: money(eur(cut.amount * n * 12, cut.currency)) }))),
        h('p.panel__fine.rf-sim__fine', t('rf_sim_fine', { plan: plan === 'max' ? 'Max' : 'Pro', cut: money(cut), rate: p.rate }))
      );
    }

    /* how it works ------------------------------------------------------ */

    function howPanel() {
      var p = program();
      function step(n, name, title, text) {
        return h('li.rf-step',
          h('span.rf-step__icon', icon(name, 18), h('span.rf-step__n', String(n))),
          h('h3.rf-step__title', title),
          h('p.rf-step__text', text));
      }
      return h('section.panel.rf-how', { 'aria-labelledby': 'rf-how-title' },
        h('div.panel__head',
          h('h2.panel__title#rf-how-title', t('rf_how')),
          h('p.panel__aside.rf-how__links',
            h('a', { href: '../affiliates/#faq' }, t('rf_faq')),
            h('a', { href: '../legal/affiliate-terms.html' }, t('rf_terms')))),
        h('ol.rf-steps',
          step(1, 'link', t('rf_step1'), t('rf_step1_text')),
          step(2, 'gift', t('rf_step2', { n: ctx.num(p.bonus) }), t('rf_step2_text')),
          step(3, 'percent', t('rf_step3', { rate: p.rate }), t('rf_step3_text'))),
        h('p.panel__fine', t('rf_how_fine', { days: p.days, min: money(p.min, true) }))
      );
    }

    /* the funnel -------------------------------------------------------- */

    function funnelPanel(d) {
      var st = d.stats;
      var weeks = d.weeks || [];
      var empty = !st.visits && !st.trials && !st.customers && !weeks.some(function (w) { return w.visits; });
      function stage(key, value, fine, conv, field) {
        return h('li.rf-stage.rf-stage--' + field,
          h('p.rf-stage__name', t(key)),
          h('p.rf-stage__num', { 'data-count-int': String(value) }, ctx.num(value)),
          h('p.rf-stage__fine', fine),
          conv ? h('p.rf-stage__conv', conv) : h('p.rf-stage__conv.rf-stage__conv--none', { 'aria-hidden': 'true' }, ' '),
          empty ? null : sparkline(weeks, field, t(key)));
      }
      var c1 = pct(st.trials, st.visits);
      var c2 = pct(st.customers, st.trials);
      return h('section.panel.rf-funnel', { 'aria-labelledby': 'rf-funnel-title' },
        h('div.panel__head',
          h('h2.panel__title#rf-funnel-title', t('rf_funnel')),
          h('p.panel__aside', t('rf_funnel_aside'))),
        h('ol.rf-stages',
          stage('rf_visits', st.visits, t('rf_visits_fine'), null, 'visits'),
          stage('rf_trials', st.trials, t('rf_trials_fine'), c1 && t('rf_conv', { pct: c1 }), 'trials'),
          stage('rf_customers', st.customers, t('rf_customers_fine'), c2 && t('rf_conv_trials', { pct: c2 }), 'customers')),
        empty ? h('p.rf-funnel__empty', t('rf_funnel_empty')) : null
      );
    }

    // Twelve columns, one a week, the current one in the accent, on a
    // hairline baseline; a tooltip on hover or touch, and the same
    // figures in a table for a reader.
    function sparkline(weeks, field, what) {
      var n = weeks.length;
      var max = Math.max.apply(null, weeks.map(function (w) { return w[field] || 0; }).concat([1]));
      var tip = h('span.rf-spark__tip', { 'aria-hidden': 'true', hidden: true });
      var cols = weeks.map(function (w, i) {
        var v = w[field] || 0;
        var pctH = v ? Math.max(7, Math.round(v / max * 100)) : 0;
        return h('span.rf-spark__col' + (i === n - 1 ? '.is-now' : '') + (v ? '' : '.is-zero'), { 'data-i': String(i) },
          h('i', { style: 'height:' + pctH + '%' }));
      });
      var plot = h('div.rf-spark__plot', { 'aria-hidden': 'true' }, cols, tip);
      function show(ev) {
        var col = ev.target.closest && ev.target.closest('.rf-spark__col');
        if (!col) return;
        var i = +col.getAttribute('data-i');
        var w = weeks[i];
        tip.textContent = t('rf_week_tip', { date: ctx.shortDate(isoDay(w.start)), n: ctx.num(w[field] || 0) });
        tip.hidden = false;
        var rect = plot.getBoundingClientRect();
        var c = col.getBoundingClientRect();
        tip.style.left = (c.left - rect.left + c.width / 2) + 'px';
        tip.classList.toggle('is-start', i < 2);
        tip.classList.toggle('is-end', i > n - 3);
        cols.forEach(function (el, k) { el.classList.toggle('is-hover', k === i); });
      }
      function hide() {
        tip.hidden = true;
        cols.forEach(function (el) { el.classList.remove('is-hover'); });
      }
      plot.addEventListener('pointerover', show);
      plot.addEventListener('pointerdown', show);
      plot.addEventListener('pointerleave', hide);
      return h('div.rf-spark',
        h('p.rf-spark__title', t('rf_weeks')),
        plot,
        // In a box of its own: a table ignores the overflow that hides it.
        h('div.sr-only', h('table',
          h('caption', t('rf_weeks_caption', { what: what })),
          h('thead', h('tr', h('th', { scope: 'col' }, t('rf_week_col')), h('th', { scope: 'col' }, what))),
          h('tbody', weeks.map(function (w) {
            return h('tr', h('td', ctx.shortDate(isoDay(w.start))), h('td', ctx.num(w[field] || 0)));
          })))));
    }

    /* the referrals ---------------------------------------------------- */

    function personLabel(r) {
      if (r.status === 'trial') return t('rf_p_trial');
      if (r.status === 'refunded') return t('rf_p_refunded');
      if (r.status === 'ended') return t('rf_p_ended');
      if (r.product && STR.en['rf_p_' + r.product]) return t('rf_p_' + r.product);
      return t('rf_p_credits');
    }

    function personIcon(r) {
      if (r.status === 'trial') return 'clock';
      if (r.product === 'lifetime') return 'infinity';
      return 'person';
    }

    // Who pays first, then who is trying, then who left; the most
    // recent first within each.
    var ORDER = { active: 0, trial: 1, ended: 2, refunded: 3 };

    function peoplePanel(d) {
      var list = (d.referrals || []).slice().sort(function (a, b) {
        var oa = ORDER[a.status] === undefined ? 4 : ORDER[a.status];
        var ob = ORDER[b.status] === undefined ? 4 : ORDER[b.status];
        return oa - ob || b.since - a.since;
      });
      var p = program();
      var customers = d.stats.customers;
      var trials = list.filter(function (r) { return r.status === 'trial'; }).length;
      var LIMIT = 6;
      var shown = R.allPeople ? list : list.slice(0, LIMIT);
      var body;
      if (!list.length) {
        var cut = example('pro_monthly') || eur(447);
        body = h('div.rf-empty',
          h('span.rf-empty__art', { 'aria-hidden': 'true' }, h('i'), h('i'), h('i')),
          h('p.rf-empty__title', t('rf_people_empty', { amount: money(cut) })),
          h('p.rf-empty__text', t('rf_people_empty_text')),
          btn(t('rf_copy_link'), { small: true, icon: 'copy', onclick: function (ev) { copyLink(d, ev.currentTarget); } }));
      } else {
        body = [
          h('ul.rf-people__list', shown.map(function (r, i) {
            var value;
            if (r.status === 'trial') value = h('span.rf-person__value.is-muted', t('rf_nothing_yet'));
            else if (r.monthly) value = h('span.rf-person__value', t('rf_per_month', { amount: money(r.monthly) }));
            else if (r.product === 'lifetime' && r.earned.amount > 0) value = h('span.rf-person__value', t('rf_once', { amount: money(r.earned) }));
            else value = h('span.rf-person__value.is-muted', money(r.earned));
            return h('li.rf-person.rf-person--' + r.status, { id: 'rf-person-' + i, tabindex: '-1' },
              h('span.rf-person__icon', icon(personIcon(r))),
              h('div.rf-person__text',
                h('p.rf-person__name', personLabel(r)),
                h('p.rf-person__meta', r.status === 'trial' ? t('rf_started', { when: ctx.ago(r.since) }) : t('rf_since', { when: sinceMonth(r.since) }))),
              h('div.rf-person__money',
                value,
                r.status !== 'trial' && r.earned.amount > 0 && (r.monthly || r.product !== 'lifetime')
                  ? h('span.rf-person__sofar', t('rf_so_far', { amount: money(r.earned) })) : null));
          })),
          list.length > LIMIT ? h('div.rf-people__more', btn(R.allPeople ? t('rf_people_less') : t('rf_people_more', { n: ctx.num(list.length) }), {
            small: true, quiet: true, onclick: function () {
              R.allPeople = !R.allPeople;
              if (R.allPeople) ctx.focus('#rf-person-' + LIMIT);
              ctx.render();
            }
          })) : null,
          d.referrals_more ? h('p.panel__fine', t('rf_people_beyond', { n: ctx.num(d.referrals_more) })) : null
        ];
      }
      void p;
      return h('section.panel.rf-people', { 'aria-labelledby': 'rf-people-title' },
        h('div.panel__head',
          h('h2.panel__title#rf-people-title', t('rf_people')),
          list.length ? h('p.panel__aside', t(customers === 1 ? 'rf_people_aside_one' : 'rf_people_aside', { n: ctx.num(customers), m: ctx.num(trials) })) : null),
        body);
    }

    /* the milestones ---------------------------------------------------- */

    function badgesPanel(d) {
      var p = program();
      var customers = d.stats.customers;
      var list = d.milestones || [];
      var next = list.filter(function (m) { return !m.reached_at; })[0];
      return h('section.panel.rf-badges', { 'aria-labelledby': 'rf-badges-title' },
        h('div.panel__head', h('h2.panel__title#rf-badges-title', t('rf_badges'))),
        h('ul.rf-badges__list', list.map(function (m, i) {
          var reached = !!m.reached_at;
          var isNext = next === m;
          var prev = i ? list[i - 1].customers : 0;
          var progress = reached ? 1 : isNext ? Math.max(0, Math.min(1, (customers - prev) / (m.customers - prev))) : 0;
          var label = m.customers === 1 ? t('rf_badge_1') : t('rf_badge_n', { n: ctx.num(m.customers) });
          var sub = reached ? t('rf_badge_reached', { date: ctx.shortDate(m.reached_at) })
            : isNext ? t('rf_badge_togo', { n: ctx.num(m.customers - customers) }) : t('rf_badge_locked');
          return h('li.rf-badge' + (reached ? '.is-reached' : isNext ? '.is-next' : '.is-locked'),
            medal(m.customers, progress, reached),
            h('p.rf-badge__name', label),
            h('p.rf-badge__sub', sub));
        })),
        h('p.panel__fine', t('rf_badges_fine', { rate: p.rate })));
    }

    // A hexagon with the count in it; reached, it is filled; the next
    // one has a ring that shows how far along it is.
    function medal(n, progress, reached) {
      var hex = 'M28 3.5 49.2 15.75v24.5L28 52.5 6.8 40.25v-24.5Z';
      var ring = 2 * Math.PI * 25;
      var kids = [
        s('path', { d: hex, 'class': 'rf-medal__hex' }),
        reached ? null : s('circle', { cx: '28', cy: '28', r: '25', 'class': 'rf-medal__track' }),
        !reached && progress > 0 ? s('circle', {
          cx: '28', cy: '28', r: '25', 'class': 'rf-medal__ring',
          'stroke-dasharray': (ring * progress).toFixed(2) + ' ' + ring.toFixed(2),
          transform: 'rotate(-90 28 28)'
        }) : null
      ];
      var text = s('text', { x: '28', y: '28', 'class': 'rf-medal__n', 'text-anchor': 'middle', 'dominant-baseline': 'central' });
      text.textContent = String(n);
      kids.push(text);
      return h('span.rf-medal', { 'aria-hidden': 'true' }, s('svg', { viewBox: '0 0 56 56', width: '56', height: '56' }, kids));
    }

    /* payouts and statements ------------------------------------------- */

    function moneyPanel(d) {
      var list = R.statements || [];
      var n = d.next_payout || { state: 'none', amount: eur(0) };
      var p = program();
      var date = ctx.date(isoDay(n.date || new Date().toISOString().slice(0, 10)));
      var nextLine = n.state === 'scheduled' ? t('rf_next_scheduled', { amount: money(n.amount), date: date })
        : n.state === 'needs_profile' ? t('rf_next_profile', { amount: money(n.amount), date: date })
          : n.state === 'below_minimum' ? t('rf_next_below', { amount: money(n.amount), left: money(eur(p.min.amount - n.amount.amount, p.min.currency)) })
            : t('rf_next_none', { min: money(p.min, true) });

      var paid = list.filter(function (st) { return st.payout; });
      var payBody = R.stError ? null : !paid.length
        ? h('p.rf-st__empty', t('rf_payouts_empty', { min: money(p.min, true) }))
        : h('ul.rf-pay', paid.map(function (st) {
          return h('li.rf-pay__row',
            h('span.rf-pay__date', ctx.date(st.payout.paid_at)),
            h('span.rf-pay__how', t(st.payout.method === 'paypal' ? 'rf_m_paypal' : 'rf_m_iban'), ' · ', h('code', st.payout.reference)),
            h('span.rf-pay__amount', money(st.payout)));
        }));
      var stBody;
      if (R.stError) {
        stBody = h('div.notice.notice--error', { role: 'alert' }, h('p', t('rf_statements_error')),
          h('p', ctx.button(t('retry'), { small: true, onclick: function () { load({ fresh: true }); } })));
      } else if (!list.length) {
        stBody = h('p.rf-st__empty', t('rf_statements_empty'));
      } else {
        stBody = h('ul.rf-st', list.map(function (st) {
          var month = monthName(st.month);
          return h('li.rf-st__row',
            h('span.rf-st__icon', icon('doc')),
            h('div.rf-st__text',
              h('p.rf-st__month', month),
              h('p.rf-st__meta',
                [t('rf_st_earned', { amount: money(st.earned) }),
                  st.cancelled && st.cancelled.amount ? t('rf_st_cancelled', { amount: money(st.cancelled) }) : null,
                  st.payout ? t('rf_st_paid', { amount: money(st.payout), date: ctx.shortDate(st.payout.paid_at) }) : null]
                  .filter(Boolean).join(' · '))),
            h('div.rf-st__actions',
              btn(t('rf_st_open'), { small: true, label: t('rf_st_open_label', { month: month }), busy: R.stBusy === st.id + ':view', onclick: function () { openStatement(st, 'view'); } }),
              btn(t('rf_st_csv'), { small: true, quiet: true, icon: 'download', label: t('rf_st_csv_label', { month: month }), busy: R.stBusy === st.id + ':csv', onclick: function () { openStatement(st, 'csv'); } })));
        }));
      }

      return h('section.panel.rf-money', { 'aria-labelledby': 'rf-money-title' },
        h('div.panel__head', h('h2.panel__title#rf-money-title', t('rf_money'))),
        h('div.rf-money__next',
          h('p.rf-money__label', t('rf_payouts_next')),
          h('p.rf-money__line', nextLine)),
        accordion('payouts', t('rf_payouts'), paid.length ? t('rf_payouts_count', { n: ctx.num(paid.length) }) : null, payBody),
        accordion('statements', t('rf_statements'), list.length ? t('rf_statements_count', { n: ctx.num(list.length) }) : null, stBody)
      );
    }

    function accordion(key, title, aside, body) {
      var open = R.open[key] !== undefined ? R.open[key] : false;
      var el = h('details.rf-acc', { open: open ? true : null },
        h('summary', h('span.rf-acc__title', title), aside ? h('span.rf-acc__aside', aside) : null, h('span.rf-acc__chev', icon('chevron', 14))),
        h('div.rf-acc__body', body));
      el.addEventListener('toggle', function () { R.open[key] = el.open; });
      return el;
    }

    function openStatement(st, how) {
      if (R.stBusy) return;
      R.stBusy = st.id + ':' + how;
      ctx.render();
      api().statement(st.id).then(function (res) {
        R.stBusy = null;
        ctx.render();
        if (how === 'csv') return save('hangarai-referral-statement-' + st.month + '.csv', new Blob([csvOf(res)], { type: 'text/csv;charset=utf-8' }));
        statementSheet(res);
      }, function (err) {
        R.stBusy = null;
        if (err.status === 401) return ctx.reload();
        ctx.toast(t(ctx.errorKey(err)));
        ctx.render();
      });
    }

    function productName(id) {
      return t(id && STR.en['rf_prod_' + id] ? 'rf_prod_' + id : 'rf_prod_credits');
    }

    function lineWhat(l) {
      return t(l.kind === 'refund' ? 'rf_k_refund' : 'rf_k_payment', { product: productName(l.product) });
    }

    // RFC 4180, a header and one row a line; amounts in euros with a
    // point, dates as ISO days, so a spreadsheet reads them anywhere.
    function csvOf(res) {
      function cell(v) {
        var text = String(v === null || v === undefined ? '' : v);
        return /[",\n\r]/.test(text) ? '"' + text.replace(/"/g, '""') + '"' : text;
      }
      var rows = [['date', 'referral', 'product', 'kind', 'net_eur', 'commission_eur', 'status']];
      (res.lines || []).forEach(function (l) {
        rows.push([new Date(l.at).toISOString().slice(0, 10), l.referral, l.product || 'credits', l.kind,
          (l.net.amount / 100).toFixed(2), (l.commission.amount / 100).toFixed(2), l.status]);
      });
      var st = res.statement;
      if (st && st.payout) {
        rows.push([new Date(st.payout.paid_at).toISOString().slice(0, 10), '', '', 'payout', '', (-st.payout.amount / 100).toFixed(2), 'paid ' + st.payout.reference]);
      }
      return '﻿' + rows.map(function (r) { return r.map(cell).join(','); }).join('\r\n') + '\r\n';
    }

    // The statement in the page's dialog, with a print view of its own.
    function statementSheet(res) {
      var dlg = document.getElementById('dlg');
      var st = res.statement;
      var month = monthName(st.month);
      if (!dlg || typeof dlg.showModal !== 'function') return save('hangarai-referral-statement-' + st.month + '.csv', new Blob([csvOf(res)], { type: 'text/csv;charset=utf-8' }));
      var back = document.activeElement;
      dlg.textContent = '';
      dlg.classList.add('dlg--wide');
      function close() {
        dlg.close();
      }
      var table = h('table.stmt__table',
        h('thead', h('tr',
          h('th', { scope: 'col' }, t('rf_st_date')),
          h('th', { scope: 'col' }, t('rf_st_what')),
          h('th', { scope: 'col' }, t('rf_st_ref')),
          h('th.num', { scope: 'col' }, t('rf_st_net')),
          h('th.num', { scope: 'col' }, t('rf_st_commission')),
          h('th', { scope: 'col' }, t('rf_st_status')))),
        h('tbody', (res.lines || []).map(function (l) {
          return h('tr' + (l.kind === 'refund' ? '.is-refund' : ''),
            h('td.stmt__date', ctx.shortDate(l.at)),
            h('td', lineWhat(l)),
            h('td.stmt__ref', h('code', l.referral)),
            h('td.num', money(l.net)),
            h('td.num.stmt__cut', money(l.commission)),
            h('td', h('span.stmt__st.stmt__st--' + l.status, t('rf_s_' + l.status))));
        })));
      var payee = res.payee;
      dlg.appendChild(h('div.stmt',
        h('div.stmt__head',
          h('div',
            h('p.stmt__issuer', t('rf_st_issuer')),
            h('h2.dlg__title#dlg-title', t('rf_st_title', { month: month })),
            payee ? h('p.stmt__payee', t('rf_st_payee', { name: payee.legal_name, country: countryName(payee.country) })) : null),
          h('button.stmt__x', { type: 'button', 'aria-label': t('rf_st_close'), onclick: close }, '×')),
        h('div.stmt__scroll', table),
        h('dl.stmt__totals',
          h('div', h('dt', t('rf_st_total')), h('dd', money(st.earned))),
          st.cancelled && st.cancelled.amount ? h('div', h('dt', t('rf_st_total_cancelled')), h('dd', '−' + money(st.cancelled))) : null,
          h('div.stmt__payout', h('dt', st.payout ? t('rf_st_payout', { date: ctx.date(st.payout.paid_at), ref: st.payout.reference }) : t('rf_st_no_payout')),
            st.payout ? h('dd', money(st.payout)) : null)),
        h('div.dlg__actions.stmt__actions',
          btn(t('rf_st_csv'), { icon: 'download', onclick: function () {
            save('hangarai-referral-statement-' + st.month + '.csv', new Blob([csvOf(res)], { type: 'text/csv;charset=utf-8' }));
          } }),
          btn(t('rf_st_print'), { primary: true, onclick: function () {
            var root = document.documentElement;
            root.classList.add('printing-statement');
            var done = function () {
              root.classList.remove('printing-statement');
              window.removeEventListener('afterprint', done);
            };
            window.addEventListener('afterprint', done);
            window.print();
            setTimeout(done, 1000);
          } }))
      ));
      dlg.setAttribute('aria-labelledby', 'dlg-title');
      dlg.removeAttribute('aria-describedby');
      dlg.onclose = function () {
        dlg.classList.remove('dlg--wide');
        dlg.onclose = null;
        if (back && document.contains(back)) back.focus();
      };
      dlg.oncancel = null;
      dlg.showModal();
      dlg.querySelector('.stmt__x').focus();
    }

    /* the payout details ------------------------------------------------ */

    function guessCountry() {
      var tag = (navigator.languages && navigator.languages[0]) || navigator.language || '';
      var m = /-([A-Z]{2})\b/i.exec(tag);
      var code = m ? m[1].toUpperCase() : lang() === 'fr' ? 'FR' : '';
      return COUNTRIES.indexOf(code) >= 0 ? code : '';
    }

    function freshForm(profile) {
      return {
        legal_name: profile ? profile.legal_name : (state.summary && state.summary.account && state.summary.account.display_name) || '',
        country: profile ? profile.country : guessCountry(),
        method: profile ? profile.method : 'iban',
        iban: '', bic: '', paypal_email: '',
        accept: !!(profile && profile.terms === TERMS)
      };
    }

    function openPayout() {
      R.payoutEdit = true;
      R.form = freshForm(R.data.payout_profile);
      R.formErrors = {};
      R.formError = null;
      ctx.focus('#rf-legal-name');
      ctx.render();
    }

    function payoutPanel(d) {
      var profile = d.payout_profile;
      var editing = R.payoutEdit || !profile;
      var body;
      if (!editing) {
        body = h('div.rf-profile',
          h('dl.rf-profile__facts',
            h('div', h('dt', t('rf_legal_name')), h('dd', profile.legal_name)),
            h('div', h('dt', t('rf_country')), h('dd', countryName(profile.country))),
            h('div', h('dt', t('rf_method')), h('dd', t(profile.method === 'paypal' ? 'rf_m_paypal' : 'rf_m_iban'))),
            h('div.rf-profile__acct', h('dt', profile.method === 'paypal' ? t('rf_paypal_email') : t('rf_iban')), h('dd', h('code', { translate: 'no' }, profile.account)))),
          h('div.rf-profile__foot',
            profile.updated_at ? h('p.rf-profile__when', t('rf_updated', { date: ctx.date(profile.updated_at) })) : null,
            btn(t('rf_payout_edit'), { small: true, icon: 'pencil', id: 'rf-payout-edit', onclick: openPayout })));
      } else {
        body = payoutForm(d);
      }
      return h('section.panel.rf-payout#rf-payout', { 'aria-labelledby': 'rf-payout-title' },
        h('div.rf-payout__side',
          h('h2.panel__title#rf-payout-title', { tabindex: '-1' }, t('rf_payout')),
          h('p.rf-payout__lede', profile ? t('rf_payout_lede') : t('rf_payout_none')),
          h('p.rf-lock', h('span.rf-lock__icon', icon('lock', 14)), h('span', t('rf_payout_lock'))),
          h('p.panel__fine.rf-tax', t('rf_tax'), ' ', h('a', { href: '../affiliates/#faq-tax' }, t('rf_faq')))),
        h('div.rf-payout__main', body)
      );
    }

    function payoutForm(d) {
      var f = R.form || (R.form = freshForm(d.payout_profile));
      var errs = R.formErrors || {};
      var names = COUNTRIES.map(function (c) { return { code: c, name: countryName(c) }; });
      var collator = new Intl.Collator(lang());
      names.sort(function (a, b) { return collator.compare(a.name, b.name); });

      function field(id, label, input, err, hint, extra) {
        return h('div.rf-field' + (err ? '.is-bad' : '') + (extra ? '.' + extra : ''),
          h('label', { 'for': id }, label),
          input,
          err ? h('p.rf-field__err', { id: id + '-err', role: 'alert' }, t(err)) : hint ? h('p.rf-field__hint', { id: id + '-hint' }, hint) : null);
      }
      function bind(key) {
        return function (ev) {
          f[key] = ev.target.type === 'checkbox' ? ev.target.checked : ev.target.value;
          if (errs[key]) {
            delete errs[key];
            var wrap = ev.target.closest('.rf-field');
            if (wrap) wrap.classList.remove('is-bad');
          }
        };
      }

      var ibanOk = f.iban && ibanValid(ibanClean(f.iban));
      var method = h('div.rf-seg', { role: 'radiogroup', 'aria-labelledby': 'rf-method-label' },
        ['iban', 'paypal'].map(function (m) {
          return h('button.rf-seg__btn', {
            type: 'button', role: 'radio', 'aria-checked': String(f.method === m), tabindex: f.method === m ? '0' : '-1',
            id: 'rf-method-' + m,
            onclick: function () {
              if (f.method === m) return;
              f.method = m;
              ctx.focus('#rf-method-' + m);
              ctx.render();
            },
            onkeydown: function (ev) {
              if (ev.key !== 'ArrowLeft' && ev.key !== 'ArrowRight' && ev.key !== 'ArrowUp' && ev.key !== 'ArrowDown') return;
              ev.preventDefault();
              f.method = f.method === 'iban' ? 'paypal' : 'iban';
              ctx.focus('#rf-method-' + f.method);
              ctx.render();
            }
          }, t(m === 'iban' ? 'rf_m_iban' : 'rf_m_paypal'));
        }));

      return h('form.rf-form', { novalidate: true, onsubmit: function (ev) { ev.preventDefault(); submitPayout(); } },
        R.formError ? h('div.notice.notice--error#rf-form-error', { role: 'alert', tabindex: '-1' }, h('p', t(R.formError))) : null,
        field('rf-legal-name', t('rf_legal_name'),
          h('input#rf-legal-name', {
            type: 'text', value: f.legal_name, autocomplete: 'name', maxlength: '140', required: true,
            'aria-invalid': errs.legal_name ? 'true' : null,
            'aria-describedby': errs.legal_name ? 'rf-legal-name-err' : 'rf-legal-name-hint', oninput: bind('legal_name')
          }), errs.legal_name, t('rf_legal_hint')),
        field('rf-country', t('rf_country'),
          h('select#rf-country', {
            autocomplete: 'country', required: true, 'aria-invalid': errs.country ? 'true' : null,
            'aria-describedby': errs.country ? 'rf-country-err' : null, onchange: bind('country')
          },
            h('option', { value: '', selected: f.country ? null : true, disabled: true }, t('rf_country_pick')),
            names.map(function (c) { return h('option', { value: c.code, selected: c.code === f.country ? true : null }, c.name); })),
          errs.country),
        h('div.rf-field',
          h('p.rf-field__label#rf-method-label', t('rf_method')),
          method),
        f.method === 'iban' ? h('div.rf-pair',
          field('rf-iban', t('rf_iban'),
            h('div.rf-iban' + (ibanOk ? '.is-ok' : ''),
              h('input#rf-iban', {
                type: 'text', value: f.iban, autocomplete: 'off', autocapitalize: 'characters', spellcheck: 'false',
                inputmode: 'text', maxlength: '42', placeholder: 'FR76 3000 6000 0112 3456 7890 189', required: true,
                'aria-invalid': errs.iban ? 'true' : null, 'aria-describedby': errs.iban ? 'rf-iban-err' : null,
                oninput: function (ev) {
                  f.iban = ev.target.value;
                  var ok = ibanValid(ibanClean(f.iban));
                  var mark = document.getElementById('rf-iban-ok');
                  if (mark) mark.hidden = !ok;
                  ev.target.parentNode.classList.toggle('is-ok', ok);
                  if (errs.iban) {
                    delete errs.iban;
                    ev.target.closest('.rf-field').classList.remove('is-bad');
                  }
                },
                onblur: function (ev) {
                  var clean = ibanClean(ev.target.value);
                  if (clean) ev.target.value = f.iban = ibanGroups(clean);
                }
              }),
              h('span.rf-iban__ok#rf-iban-ok', { hidden: ibanOk ? null : true }, icon('check', 14), t('rf_iban_ok'))),
            errs.iban, null, 'rf-field--grow'),
          field('rf-bic', h('span', t('rf_bic'), ' ', h('span.rf-field__opt', t('rf_optional'))),
            h('input#rf-bic', {
              type: 'text', value: f.bic, autocomplete: 'off', autocapitalize: 'characters', spellcheck: 'false',
              maxlength: '11', placeholder: 'AGRIFRPP', 'aria-invalid': errs.bic ? 'true' : null, oninput: bind('bic')
            }), errs.bic, null, 'rf-field--bic'))
          : field('rf-paypal', t('rf_paypal_email'),
            h('input#rf-paypal', {
              type: 'email', value: f.paypal_email, autocomplete: 'email', required: true,
              'aria-invalid': errs.paypal_email ? 'true' : null, 'aria-describedby': errs.paypal_email ? 'rf-paypal-err' : null,
              oninput: bind('paypal_email')
            }), errs.paypal_email),
        h('div.rf-check' + (errs.accept ? '.is-bad' : ''),
          h('input#rf-accept', { type: 'checkbox', checked: f.accept ? true : null, 'aria-invalid': errs.accept ? 'true' : null, 'aria-describedby': errs.accept ? 'rf-accept-err' : null, onchange: bind('accept') }),
          h('label', { 'for': 'rf-accept' }, acceptLabel()),
          errs.accept ? h('p.rf-field__err#rf-accept-err', { role: 'alert' }, t(errs.accept)) : null),
        h('div.rf-form__actions',
          d.payout_profile ? btn(t('cancel'), { quiet: true, onclick: function () {
            R.payoutEdit = false;
            R.formErrors = {};
            R.formError = null;
            ctx.focus('#rf-payout-edit');
            ctx.render();
          } }) : null,
          btn(t('rf_payout_save'), { primary: true, submit: true, icon: 'lock', busy: R.formBusy }))
      );
    }

    // "I accept the affiliate terms." with the terms as a link.
    function acceptLabel() {
      var text = t('rf_accept', { terms: '\u0000' });
      var parts = text.split('\u0000');
      return [parts[0], h('a', { href: '../legal/affiliate-terms.html', target: '_blank', rel: 'noopener' }, t('rf_accept_terms')), parts[1] || ''];
    }

    function submitPayout() {
      var f = R.form;
      var errs = {};
      var name = String(f.legal_name || '').trim();
      if (name.length < 2) errs.legal_name = 'rf_f_name';
      if (!f.country) errs.country = 'rf_f_country';
      var body = { legal_name: name, country: f.country, method: f.method, terms: TERMS };
      if (f.method === 'iban') {
        var iban = ibanClean(f.iban);
        if (!ibanValid(iban)) errs.iban = 'err_iban_invalid';
        body.iban = iban;
        var bic = ibanClean(f.bic);
        if (bic && !/^[A-Z0-9]{8}([A-Z0-9]{3})?$/.test(bic)) errs.bic = 'err_bad_request';
        if (bic) body.bic = bic;
      } else {
        var mail = String(f.paypal_email || '').trim();
        if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(mail)) errs.paypal_email = 'err_paypal_invalid';
        body.paypal_email = mail;
      }
      if (!f.accept) errs.accept = 'err_terms_needed';
      R.formErrors = errs;
      R.formError = null;
      var firstBad = ['legal_name', 'country', 'iban', 'bic', 'paypal_email', 'accept'].filter(function (k) { return errs[k]; })[0];
      if (firstBad) {
        ctx.focus({ legal_name: '#rf-legal-name', country: '#rf-country', iban: '#rf-iban', bic: '#rf-bic', paypal_email: '#rf-paypal', accept: '#rf-accept' }[firstBad]);
        return ctx.render();
      }
      R.formBusy = true;
      ctx.render();
      api().setPayoutProfile(body).then(function (res) {
        R.formBusy = false;
        R.payoutEdit = false;
        R.form = null;
        R.data.payout_profile = res.payout_profile;
        if (R.data.next_payout && R.data.next_payout.state === 'needs_profile') R.data.next_payout.state = 'scheduled';
        ctx.toast(t('rf_payout_saved'));
        ctx.focus('#rf-payout-edit');
        ctx.render();
      }, function (err) {
        R.formBusy = false;
        if (err.status === 401) return ctx.reload();
        var key = ctx.errorKey(err);
        if (err.code === 'iban_invalid') R.formErrors = { iban: key };
        else if (err.code === 'paypal_invalid') R.formErrors = { paypal_email: key };
        else if (err.code === 'terms_needed') R.formErrors = { accept: key };
        else R.formError = key;
        ctx.focus(R.formError ? '#rf-form-error' : err.code === 'iban_invalid' ? '#rf-iban' : err.code === 'paypal_invalid' ? '#rf-paypal' : '#rf-accept');
        ctx.render();
      });
    }

    /* ── 4. the motion ─────────────────────────────────────────── */

    // Once, the first time the figures are on screen: the big number
    // counts up, the meter fills, the columns rise, the reached
    // milestones land. Only transform, opacity and the figures' text,
    // the text held at its final width so nothing moves around it.
    function after(root) {
      if (!R.data || R.played || !root) return;
      R.played = true;
      var gsap = window.gsap;
      if (reduced || !gsap) return;
      var hero = root.querySelector('.rf-hero__num');
      if (hero) {
        var final = hero.textContent;
        var target = +hero.getAttribute('data-count');
        var currency = hero.getAttribute('data-currency') || 'EUR';
        var prefix = hero.getAttribute('data-prefix') || '';
        hero.style.minWidth = hero.getBoundingClientRect().width + 'px';
        var show = { v: 0 };
        gsap.to(show, {
          v: target, duration: target > 100000 ? 1.6 : 1.25, ease: 'power3.out', delay: 0.12,
          onUpdate: function () { hero.textContent = prefix + money(eur(show.v, currency)); },
          onComplete: function () { hero.textContent = final; hero.style.minWidth = ''; }
        });
      }
      var fill = root.querySelector('.rf-meter__track i');
      if (fill) gsap.fromTo(fill, { scaleX: 0 }, { scaleX: +fill.getAttribute('data-ratio'), duration: 1.1, ease: 'power3.out', delay: 0.35 });
      Array.prototype.forEach.call(root.querySelectorAll('.rf-stage__num'), function (el, i) {
        var to = +el.getAttribute('data-count-int');
        if (!to) return;
        var text = el.textContent;
        el.style.minWidth = el.getBoundingClientRect().width + 'px';
        var c = { v: 0 };
        gsap.to(c, {
          v: to, duration: 1.1, ease: 'power3.out', delay: 0.25 + i * 0.08,
          onUpdate: function () { el.textContent = ctx.num(Math.round(c.v)); },
          onComplete: function () { el.textContent = text; el.style.minWidth = ''; }
        });
      });
      Array.prototype.forEach.call(root.querySelectorAll('.rf-spark__plot'), function (plot, k) {
        var bars = plot.querySelectorAll('i');
        if (bars.length) gsap.from(bars, { scaleY: 0, transformOrigin: '50% 100%', duration: 0.7, ease: 'power3.out', stagger: 0.03, delay: 0.3 + k * 0.08 });
      });
      var medals = root.querySelectorAll('.rf-badge.is-reached .rf-medal, .rf-badge.is-next .rf-medal');
      if (medals.length) gsap.from(medals, { scale: 0.6, opacity: 0, duration: 0.6, ease: 'back.out(2)', stagger: 0.09, delay: 0.45 });
    }

    return {
      view: view,
      load: load,
      after: after,
      reset: function () {
        R.data = null;
        R.statements = null;
        R.played = false;
        R.payoutEdit = false;
        R.form = null;
        R.codeEdit = false;
      }
    };
  }

  window.HangarReferral = { STR: STR, create: create };
})();
