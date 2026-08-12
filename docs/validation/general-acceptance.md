# Aceptacion general

## Alcance

NIR-089 consolida los owners ejecutables de las capacidades finales sin crear
una segunda suite funcional paralela. La prueba
`every_final_capability_has_one_stable_executable_owner` exige una entrada unica
por capacidad y comprueba que el target y la funcion `#[test]` o
`#[tokio::test]` sigan existiendo.

La prueba
`calendar_variant_simulation_and_narrative_derivation_remain_read_only` agrega
un recorrido transversal pequeno: abre un mundo con calendario, crea una
variante, ejecuta una simulacion determinista sobre esa variante y deriva su
timeline. La revision canonica no cambia y ninguna salida se confirma ni se
escribe automaticamente.

## Matriz trazable

| Capacidad final | Owner ejecutable principal |
|---|---|
| Fundamento | `vertical_slice::mine_collapse_proposal_is_reviewed_committed_atomically_and_undone_after_reopen` |
| Retrieval | `retrieval_benchmark::hybrid_active_path_meets_the_nir_053_gate` |
| Snapshots | `snapshot_import::nir_058_hybrid_retrieval_and_snapshot_round_trip_preserve_authority_and_human_selection` |
| Deep review | `unit::deep_review::disagreement_requires_a_sourced_decision_point_before_standard_review` |
| Lore | `unit::lore_import::nir_070_offline_multipage_import_commits_only_reviewed_provenance_and_undoes_after_reopen` |
| Variants | `phase10_variants::variants_isolate_heads_history_reopen_stale_and_undo` |
| History | `phase10_variants::revision_history_follows_only_the_observed_variant_lineage` |
| Merge | `phase10_variants::compare_and_limited_merge_use_ids_and_leave_source_untouched` |
| Calendar | `phase10_variants::calendar_is_scoped_by_revision_variant_snapshot_and_undo_without_changing_ticks` |
| Simulation | `simulation::scenario_lifecycle_uses_its_variant_revision_and_never_changes_canon` |
| Narrative | `narrative::narrative_derivations_are_scoped_deterministic_bounded_and_read_only` |
| Internal document | `unit::ai::internal_document_is_perspective_scoped_referenced_and_stored_only_for_review` |
| Continuity | `unit::ai::narrative_continuity_is_read_only_then_preserves_alternatives_and_sources_in_standard_review` |
| Provider | `nirmata-ai::capabilities::provider_boundary_stays_concrete_without_marketplace_abstraction` y la matriz de [`provider-gate.md`](provider-gate.md) |

Los owners especializados siguen siendo la autoridad de regresion. La prueba
general solo cierra trazabilidad y el cruce read-only viable; no copia fixtures
masivas ni reemplaza los gates de cada modulo.

## Comandos

```powershell
cargo test -p nirmata-app --test general_acceptance
cargo test -p nirmata-ai capabilities
```

## Resultado final

El 12 de agosto de 2026, aceptación general pasó 2/2, frontend safety 9/9,
desktop 14/14 y `cargo nextest run --workspace --no-fail-fast` pasó 256/256
pruebas offline; 1 smoke test de red quedó omitido. Frontend build, formato Rust
y desktop build también pasaron.
