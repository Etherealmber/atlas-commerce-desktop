# Demande backend — clôture de caisse concurrente et idempotence

Dans `worker/routes/cashShifts.ts` (révision examinée `3b0e8c63e10c6efdfd2f21cecaf1a1f98a81458a`), l'ajout de mouvement vérifie l'état puis effectue une insertion séparée. La clôture lit la session et les mouvements, calcule le résumé, puis lance un batch et écrit l'audit. Une écriture concurrente peut intervenir entre la lecture et la clôture, et une réponse perdue ne permet pas au client de savoir si la mutation a réussi.

Merci de vérifier ces interleavings et de proposer une correction documentée :
- vérifier et modifier l'état avec une révision transactionnelle, empêcher tout mouvement de s'insérer après la clôture, et produire un instantané cohérent des mouvements et totaux ;
- vérifier le résultat de la mise à jour conditionnelle de clôture avant de signaler le succès et éviter les duplications d'instantanés en cas de deux clôtures simultanées ;
- ajouter une clé d'idempotence bornée par identité et corps pour l'ouverture, les mouvements et la clôture, avec réponse stable en cas de répétition et conflit explicite si le corps diffère ;
- rendre l'audit cohérent avec le résultat confirmé et documenter les états d'erreur et les garanties D1 ;
- tester mouvement simultané à la clôture, clôtures concurrentes, interruption à chaque étape et répétition après réponse perdue.

Ne pas déployer sans autorisation. En attendant, le desktop avertit l'utilisateur et ne relance pas automatiquement les mutations.
