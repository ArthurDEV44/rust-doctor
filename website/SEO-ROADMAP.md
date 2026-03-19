# Feuille de route SEO

## Semaine 1 (Critique)

- [ ] Vérifier que le DNS de `rust-doctor.dev` pointe vers le déploiement Vercel (ou ajouter un `metadataBase` piloté par variable d'environnement)
- [ ] Pré-optimiser les images sources à 512x512px (~50 Ko chacune au lieu de 3,5 Mo)
- [ ] S'inscrire à Google Search Console et soumettre le sitemap
- [ ] Soumettre rust-doctor sur `analysis-tools.dev/tag/rust` et `awesome-static-analysis`

## Semaines 2-3 (Priorité haute)

- [ ] Ajouter un en-tête `Content-Security-Policy` dans `next.config.ts`
- [ ] Ajouter le tag de vérification Google Search Console via `metadata.verification.google`
- [ ] Envisager l'ajout de `schema-dts` pour la validation typée des schémas JSON-LD
- [ ] Lancer Lighthouse CI et mettre en place un suivi des Core Web Vitals réels

## Mois 2 (Priorité moyenne)

- [ ] Ajouter une route `/docs` ou `/blog` pour du contenu comparatif ("rust-doctor vs clippy")
- [ ] Publier un guide "Comment mesurer la qualité du code Rust" (cible les questions PAA)
- [ ] Étendre le sitemap au fur et à mesure que des pages sont ajoutées
- [ ] Ajouter des entrées `BreadcrumbList` pour les nouvelles routes

## En continu

- [ ] Mettre à jour la date `lastModified` dans `sitemap.ts` à chaque modification de contenu
- [ ] Surveiller la position SERP pour "rust code health score"
- [ ] Suivre les citations dans les AI Overviews via Search Console
- [ ] Maintenir `llms.txt` à jour au fil des nouvelles fonctionnalités
