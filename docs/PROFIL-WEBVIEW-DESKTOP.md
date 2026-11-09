# Profil candidat WebView desktop 1 — audit, non implémenté

Audit du code web/Worker Atlas Commerce à `cf951397553dc8749f15c96b33abf02d4a360e35`, et du client desktop de la PR #68. Ce profil ne modifie pas les réponses ni le champ `flow` de `/api/auth/desktop/v1`, et ne constitue pas une affirmation de compatibilité des WebViews. Aucune release ni déploiement.

## Verdict et limites

Le contrat [v1](https://github.com/Etherealmber/atlas-commerce/blob/cf951397553dc8749f15c96b33abf02d4a360e35/docs/desktop-browser-auth-v1.md) sépare les appels natifs `capabilities`, `requests`, `poll`, `exchange`, `cancel` de l'interface web `prepare`/`authorize`. Il ne requiert ni callback ni lecture de token web par le natif. **Au niveau du protocole**, utiliser une navigation HTTPS de premier niveau dans une WebView est plausible sans nouvel endpoint. Le document impose toutefois actuellement un navigateur système : une adoption WebView doit être décidée et documentée, pas décrite comme une fonctionnalité v1 déjà validée. Aucun changement serveur indispensable n'est démontré par cette seule lecture.

Le client actuel ouvre `authorization_url` avec `open::that` et garde la session v1 uniquement en mémoire. Il n'a ni WebView, ni coffre système pour cette session. La persistance et l'isolation par serveur et compte restent à concevoir et tester. Les sessions classiques utilisent un stockage différent ; ne pas prétendre que ce stockage est un coffre chiffré.

## Constatations vérifiées dans le code

- [`LoginDesktop.tsx`](https://github.com/Etherealmber/atlas-commerce/blob/cf951397553dc8749f15c96b33abf02d4a360e35/src/pages/LoginDesktop.tsx) conserve le contexte du fragment dans `sessionStorage`, efface le fragment de l'historique et redirige vers `/login` si nécessaire. L'approbation requiert une comparaison cochée et une action explicite.
- [`desktopApproval.ts`](https://github.com/Etherealmber/atlas-commerce/blob/cf951397553dc8749f15c96b33abf02d4a360e35/src/lib/desktopApproval.ts) borne et valide ce contexte, utilise un retour fixe et bloque les redirections des requêtes web de consentement. Un stockage inaccessible fait échouer le parcours.
- [`Login.tsx`](https://github.com/Etherealmber/atlas-commerce/blob/cf951397553dc8749f15c96b33abf02d4a360e35/src/pages/Login.tsx) suit `needs_otp` puis `/api/auth/verify-otp`. L'OTP et le challenge ne sont pas facultatifs du fait de l'hébergement WebView. Un formulaire perdu après fermeture/rechargement peut nécessiter de recommencer ; la demande native continue d'expirer à son échéance absolue.
- Le [Worker](https://github.com/Etherealmber/atlas-commerce/blob/cf951397553dc8749f15c96b33abf02d4a360e35/worker/index.ts) a des heuristiques de client natif qui peuvent éviter Turnstile ; une WebView doit suivre le chemin web normal, sans marqueurs natifs ni bridge Capacitor. Sa CSP autorise notamment les scripts, connexions et frames Turnstile ; cela ne garantit pas le fonctionnement du challenge dans un moteur particulier.
- [`config.ts`](https://github.com/Etherealmber/atlas-commerce/blob/cf951397553dc8749f15c96b33abf02d4a360e35/src/config.ts) donne priorité à `VITE_API_URL` puis détecte Capacitor/HTTPS localhost : vérifier la configuration réelle du serveur pour éviter un appel vers une autre origine. Ne pas héberger `/login` sous localhost/file/custom scheme.
- [`auth.ts`](https://github.com/Etherealmber/atlas-commerce/blob/cf951397553dc8749f15c96b33abf02d4a360e35/src/auth.ts) utilise `localStorage` pour la session web et des comptes ; [`fingerprint.ts`](https://github.com/Etherealmber/atlas-commerce/blob/cf951397553dc8749f15c96b33abf02d4a360e35/src/fingerprint.ts) emploie local/session storage et teste surtout `PublicKeyCredential` avant `navigator.credentials`. Effacer seulement les cookies est insuffisant ; la présence de l'API ne démontre pas une passkey utilisable.
- Les tests web de consentement utilisent un stub de login ; ils ne démontrent pas un parcours réel OTP/Turnstile en WebView. Les tests Worker existants couvrent notamment contexte, preuves, limites et échange concurrent ; ils n'ont **pas été réexécutés** dans cet audit.

## Profil d'intégration proposé (à implémenter côté desktop)

1. Valider l'origine HTTPS choisie, son port, le certificat et l'absence de credentials, query, fragment ou chemin non racine. Créer state/verifier CSPRNG et fingerprint desktop ; n'exposer à la WebView ni verifier ni jeton desktop. Conserver séparément le fingerprint web.
2. Appeler v1 nativement sans suivre les redirections. Vérifier la découverte strictement (`system_browser_pkce_polling` reste inchangé), puis l'origine, le contexte et l'URL d'autorisation exacte ; afficher en dehors du contenu web serveur et code de comparaison. Charger directement l'URL validée en navigation principale HTTPS. Le fragment v1 contient déjà le state ; ne pas le journaliser.
3. Garder **le même** contexte web isolé pendant login, OTP et consentement. Pas d'injection pour lire les secrets, d'interception de corps, de bridge générique ou d'host object. Autoriser les seules navigations principales nécessaires sur l'origine choisie ; refuser ou demander confirmation explicite pour toute sortie, popup, schéma externe, téléchargement et permission. Cette règle ne doit pas bloquer aveuglément les iframes et sous-ressources nécessaires au challenge. TLS reste vérifié. L'implémentation doit préciser et tester ses exceptions exactes.
4. Poll/cancel/exchange restent natifs. Respecter les limites du contrat (300 s, consentement ≤120 s, au moins 5 s entre polls, 60 polls acceptés, timeouts et Retry-After). Seul un échange validé fait foi ; ni navigation, titre, ni message JS ne constitue une preuve. Stocker la session native uniquement dans le coffre système, par serveur et compte, avec comportement explicite si indisponible.
5. Annulation/fermeture/changement serveur : arrêter le polling et tenter cancel sans bloquer l'interface ; ignorer les résultats tardifs. Un échange peut déjà avoir consommé la demande ou émis un jeton : l'annulation n'est pas une révocation et l'issue d'une requête interrompue peut être inconnue. Ne pas promettre une révocation ou supprimer arbitrairement un appareil. En cas d'échange incertain, repartir avec une nouvelle demande après avertissement.
6. Détruire le contexte temporaire après chaque issue : profil WebView2 isolé, supprimé après libération effective ; `WKWebsiteDataStore.nonPersistent()` ; contexte/data manager WebKitGTK éphémère adapté à la version. Purger aussi stockage web et caches sans toucher au navigateur système ou à la session native. Le nettoyage local ne révoque pas une session web côté serveur.
7. Si WebAuthn ou Turnstile échoue, expliquer l'échec, conserver la politique serveur et proposer explicitement le navigateur système. Annuler puis recréer une demande lors du repli est l'option simple ; ne jamais copier la session WebView dans le navigateur. Le support effectif de passkeys/challenges reste à valider pour chaque moteur.

## Diagnostic de la capture

La chaîne « Réponse de connexion invalide. » provient de `crates/ui/src/browser_auth.rs` : JSON d'enveloppe illisible, champs de création invalides, contexte/intervalle du poll invalide ou statut inattendu. La capture seule ne distingue pas HTML de proxy, JSON incorrect, mauvaise version réellement déployée, contexte non conforme ou autres causes. Aucun corps ni URL complète ne doit être collecté. Pour un futur diagnostic : étape fixe, statut HTTP, catégorie Content-Type, catégorie de schéma et durée seulement ; aucun secret ni corps brut.

## Recette réelle (non exécutée)

Pour chaque plateforme, consigner uniquement version OS et moteur : Windows/WebView2, macOS/WKWebView, Linux/distribution/WebKitGTK et backend graphique. Sur serveur HTTPS de test et comptes sans données réelles : profil vierge, créer la demande, comparer serveur/code, effectuer Turnstile, mot de passe et OTP, vérifier qu'aucune session native n'existe avant consentement, approuver, poll/exchange natifs et vérifier une requête authentifiée. Vérifier isolation et nettoyage, puis refus, fermeture, expiration, réseau interrompu, stockage bloqué, changement d'origine et résultat d'échange incertain ; tester rejeu, passkey disponible/indisponible et repli navigateur système. Ne jamais enregistrer captures ou logs contenant secrets, données privées ou lien complet.

| Plateforme | Résultat réel |
|---|---|
| Windows / WebView2 | Non exécuté |
| macOS / WKWebView | Non exécuté |
| Linux / WebKitGTK | Non exécuté |

Contenu distant dans une WebView reste une frontière de confiance sensible ; une lecture du code ou un test unitaire ne remplace pas la validation de l'hôte, du stockage, des challenges et des passkeys sur les appareils réels.
