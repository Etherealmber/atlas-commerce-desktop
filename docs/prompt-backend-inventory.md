# Demande backend — transferts entre entrepôts atomiques

Contexte : le client desktop consomme le Worker Atlas Commerce. Dans `worker/routes/inventoryOps.ts` au commit `3b0e8c63e10c6efdfd2f21cecaf1a1f98a81458a`, `POST /api/inventory/transfer` appelle successivement `adjustLocationStock` pour retirer la quantité de la source, puis l'ajouter à la destination, puis `applyStockDelta` pour l'historique. Examiner ces écritures et les effets de bord associés : une erreur ou interruption entre les étapes pourrait laisser une mutation partielle. Le client ne doit pas supposer qu'une réponse perdue signifie échec ni relancer automatiquement.

Merci de proposer et implémenter une opération transactionnelle côté Worker/D1 :

1. Vérifier produit suivi, quantité finie strictement positive, emplacements actifs et distincts, droits et stock disponible ; appliquer la même politique que les autres mouvements.
2. Regrouper dans une seule transaction D1 les contrôles concurrents, le débit, le crédit, la cohérence du total produit, la révision de placement et l'écriture d'historique. Toute erreur doit laisser l'état inchangé.
3. Ajouter une clé d'idempotence fournie par le client, bornée et liée à l'identité, à l'origine et au corps canonique. Une répétition identique retourne le même résultat ; une clé réutilisée avec un corps différent est refusée. Documenter durée et comportement après expiration.
4. Invalider les caches uniquement après succès et documenter la réponse d'une opération déjà exécutée, avec codes d'erreur stables sans données sensibles.
5. Tester concurrence sur un même stock, échec à chaque étape, réponse réseau perdue puis répétition, réutilisation contradictoire de clé, rôles et locations invalides. Vérifier la conservation du total, la non-négativité selon politique et l'unicité de l'historique.

## Comptage et cohérence des lectures

`POST /api/inventory/cycle-count` lit le stock théorique, calcule un delta, puis `applyStockDelta` relit et modifie le produit. Un mouvement concurrent entre ces étapes peut empêcher le résultat final de correspondre au comptage ; les écritures de stock, placements et historique ne sont pas toutes atomiques. Prévoir une transaction avec comparaison de révision/version, refus explicite en cas de concurrence et clé d'idempotence. Tester les interleavings, l'échec partiel et la répétition après réponse perdue. Fournir aussi une lecture de vérification non mise en cache : les listes produits et emplacements actuelles peuvent servir des données en cache après une mutation ou une réponse perdue.

Ne pas déployer ni activer de modification en production sans validation explicite. Le desktop n'active pas ici de bouton de transfert tant que cette garantie n'est pas établie.
