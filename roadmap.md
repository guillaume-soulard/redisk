# Redis Roadmap - Redisk

Ce document liste les commandes Redis standard et indique leur statut d'implémentation dans Redisk.

## Légende
- ✅ : Implémenté
- 🚧 : En cours / Partiellement implémenté
- ❌ : Non implémenté

---

## Commandes de Clés (Generic)
- ✅ **DEL** : Supprime une ou plusieurs clés.
- ✅ **EXISTS** : Vérifie si une clé existe.
- ✅ **EXPIRE** : Définit un délai d'expiration sur une clé.
- ✅ **KEYS** : Trouve toutes les clés correspondant à un motif.
- ✅ **SELECT** : Sélectionne la base de données courante.
- ✅ **MOVE** : Déplace une clé vers une autre base de données.
- ✅ **PERSIST** : Supprime le délai d'expiration d'une clé.
- ✅ **PTTL** : Renvoie le temps restant avant expiration en millisecondes.
- ✅ **RANDOMKEY** : Renvoie une clé aléatoire.
- ✅ **RENAME** : Renomme une clé.
- ✅ **SCAN** : Itère sur l'ensemble des clés.
- ✅ **TTL** : Renvoie le temps restant avant expiration en secondes.
- ✅ **TYPE** : Détermine le type de données stocké à une clé.

## Chaînes de caractères (Strings)
- ❌ **APPEND** : Ajoute de la donnée à une clé.
- ❌ **DECR** : Décrémente la valeur d'une clé.
- ✅ **GET** : Récupère la valeur d'une clé.
- ❌ **GETSET** : Définit la valeur d'une clé et renvoie son ancienne valeur.
- ❌ **INCR** : Incrémente la valeur d'une clé.
- ❌ **MGET** : Récupère les valeurs de plusieurs clés.
- ❌ **MSET** : Définit plusieurs clés à plusieurs valeurs.
- ✅ **SET** : Définit la valeur d'une clé (Supporte l'option `EX` pour le TTL).
- ❌ **STRLEN** : Récupère la longueur d'une valeur de clé.

## Listes (Lists)
- ❌ **LINDEX** : Récupère un élément par son index.
- ❌ **LINSERT** : Insère un élément avant ou après un autre.
- ❌ **LLEN** : Récupère la longueur de la liste.
- ❌ **LPOP** : Supprime et renvoie le premier élément.
- ❌ **LPUSH** : Ajoute un élément au début.
- ❌ **LRANGE** : Récupère une plage d'éléments.
- ❌ **LREM** : Supprime des éléments.
- ❌ **LSET** : Définit la valeur d'un élément par son index.
- ❌ **LTRIM** : Tronque la liste.
- ❌ **RPOP** : Supprime et renvoie le dernier élément.
- ❌ **RPUSH** : Ajoute un élément à la fin.

## Ensembles (Sets)
- ❌ **SADD** : Ajoute un ou plusieurs membres.
- ❌ **SCARD** : Récupère le nombre de membres.
- ❌ **SDIFF** : Différence entre plusieurs ensembles.
- ❌ **SINTER** : Intersection entre plusieurs ensembles.
- ❌ **SISMEMBER** : Vérifie si un membre appartient à l'ensemble.
- ❌ **SMEMBERS** : Récupère tous les membres.
- ❌ **SREM** : Supprime un ou plusieurs membres.
- ❌ **SUNION** : Union de plusieurs ensembles.

## Ensembles Triés (Sorted Sets)
- ❌ **ZADD** : Ajoute un membre avec un score.
- ❌ **ZCARD** : Récupère le nombre de membres.
- ❌ **ZCOUNT** : Compte les membres avec un score dans une plage.
- ❌ **ZRANGE** : Renvoie une plage de membres par index.
- ❌ **ZREM** : Supprime un ou plusieurs membres.
- ❌ **ZSCORE** : Récupère le score d'un membre.

## Hashes
- ❌ **HDEL** : Supprime un ou plusieurs champs.
- ❌ **HEXISTS** : Vérifie si un champ existe.
- ❌ **HGET** : Récupère la valeur d'un champ.
- ❌ **HGETALL** : Récupère tous les champs et valeurs.
- ❌ **HINCRBY** : Incrémente la valeur d'un champ numérique.
- ❌ **HKEYS** : Récupère tous les noms de champs.
- ❌ **HLEN** : Récupère le nombre de champs.
- ❌ **HSET** : Définit la valeur d'un champ.
- ❌ **HVALS** : Récupère toutes les valeurs.

## Serveur / Administration
- ✅ **COMPACT** : Commande personnalisée pour la compaction du stockage.
- ❌ **FLUSHALL** : Supprime toutes les clés de toutes les bases.
- ❌ **FLUSHDB** : Supprime toutes les clés de la base courante.
- ❌ **INFO** : Récupère des informations et statistiques sur le serveur.
- ✅ **PING** : Vérifie la disponibilité du serveur.
- ❌ **SAVE** : Sauvegarde synchrone sur le disque.
- ❌ **SHUTDOWN** : Arrête le serveur.
