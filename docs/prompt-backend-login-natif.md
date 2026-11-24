# Demande backend : connexion native desktop sécurisée

Le backend actuel documente l’approbation téléphone v2, mais annonce explicitement
`password_otp` et `windows_passkey` indisponibles. Ne pas changer les parcours
web, le QR historique ou v1, et ne pas contourner Turnstile.

1. Auditer les flux existants, la protection anti-abus et les associations WebAuthn.
2. Proposer un contrat versionné pour mot de passe et OTP natifs uniquement si
   une protection anti-abus native fiable et vérifiable côté serveur existe.
   Sinon laisser la capacité indisponible avec une raison stable. Ne jamais
   considérer un en-tête, une empreinte ou une affirmation du client comme
   attestation. Prévoir budgets persistants par compte/IP/appareil, expiration,
   révocation, contrôles d'origine, réponses génériques et protection contre
   l'énumération ; ne journaliser ni secrets ni corps d'authentification.
3. Pour les passkeys Windows, définir et valider une association RP/origine
   officielle avec Windows WebAuthn avant d'activer la capacité. Ne pas prétendre
   qu'une association web déjà configurée suffit à un client natif.
4. Conserver HTTPS strict, liaison à l'origine et au port, PKCE S256, état
   aléatoire, expiration, annulation et échange unique ; isoler les sessions par
   serveur. Documenter les erreurs et limites, et fournir des tests d'intégration
   contre le vrai Worker (rejeu, preuve invalide, origine, expiration, refus,
   pertes réseau, concurrence, quotas, migration et compatibilité v1/QR).
5. Documenter le déploiement, les indicateurs publics de capacités et les options
   sécurisées d'activation ; livrer les résultats des tests. Ne pas annoncer la
   méthode disponible avant que tous les prérequis soient réellement réunis.

Pour l’approbation téléphone v2 existante, vérifier séparément la configuration
`DESKTOP_NATIVE_V2_DISCOVERY`, `DESKTOP_NATIVE_V2_PHONE_APPROVAL`, la liaison D1
et la politique IP/proxy fiable. Une valeur indisponible en découverte n'est pas
une preuve que le code n'est pas déployé. L'application Android native doit
accepter explicitement l'ancien QR et le QR v2 ; le navigateur mobile utilise une
session web distincte.

## Historique Atlas Agent : synchronisation sans écrasement

L'API actuelle `/api/admin/agent/threads/:id` remplace tout le tableau des
messages à chaque PUT, sans révision, et peut renvoyer `storage: local_only`
avec un succès HTTP. Deux clients peuvent donc écraser leurs modifications.

- Ajouter, sans casser les clients web existants, une révision opaque par
  conversation et une écriture conditionnelle (`If-Match` ou équivalent) ;
  renvoyer un conflit stable en cas de modification concurrente. Autoriser une
  création conditionnelle et donner une vraie indication de persistance.
- Garder l'isolation par utilisateur vérifié, limiter taille et nombre de
  messages, contrôler IDs et entrées, et ne jamais journaliser les contenus ou
  secrets. Préserver les règles de suppression, de chiffrement et d'expiration.
- Documenter l'ordre, la pagination, l'idempotence et la stratégie de fusion ;
  tester clients concurrents, tentatives entre comptes, indisponibilité du
  stockage, reprise, anciennes versions et conflits. Aucun client ne doit
  interpréter `local_only` comme une synchronisation réussie.
