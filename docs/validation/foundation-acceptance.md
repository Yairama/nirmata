# Aceptacion del fundamento funcional

**Estado:** aceptado el 7 de agosto de 2026.

## Frontera ejecutable

La prueba
`foundation_acceptance_traverses_frontend_ipc_commands_and_persisted_app_workflow`
usa el runtime mock oficial de Tauri y envia mensajes IPC a los comandos reales.
Sin agregar un framework E2E, crea y reabre un archivo `.nirmata`, recorre los
formularios create/update de todos los tipos del canon, busca, abre URI, carga
contexto y VFS, consulta estado de credencial, revisa, confirma, detecta stale,
revalida, audita, deshace y vuelve a abrir el proyecto.

Las llamadas al proveedor permanecen en pruebas offline con transportes y
respuestas grabadas. La aceptacion no lee `.env`, no ejecuta el smoke test
ignorado y no realiza solicitudes de red.

## Matriz de Definition of Done

Todos los owners Rust siguientes se ejecutan en el mismo
`cargo nextest run --workspace`; la seguridad de presentacion se ejecuta con el
test Node indicado al final.

| DoD | Evidencia ejecutable principal |
|---|---|
| 1 | `foundation_acceptance_traverses_frontend_ipc_commands_and_persisted_app_workflow`, `creates_closes_and_reopens_world_from_disk`, `failed_migration_rolls_back_schema_and_version` |
| 2 | La aceptacion IPC crea y actualiza World, Rule, Entity, Relation, Goal, Event, Claim y Document; `round_trips_lists_and_updates_every_canon_aggregate` verifica persistencia completa. |
| 3 | El fixture IPC conserva intervalo incierto, perspectiva y negacion; `deterministic_regressions_block_invalid_state_and_preserve_allowed_cases` y las suites de core cubren tiempo, causalidad, goals, desconocido y claims. |
| 4 | La aceptacion IPC confirma solo reviews tipadas; `mine_collapse_proposal_is_reviewed_committed_atomically_and_undone_after_reopen` y `commit_rolls_back_canon_and_revision_on_constraint_failure` verifican atomicidad multioperacion. |
| 5 | La aceptacion IPC inspecciona before/after, ejecuta undo y reabre; `complete_manual_workflow_covers_replacement_stale_commit_rollback_audit_and_undo` cubre decisiones y `case_34_intentional_exception_keeps_conflict_waiver_and_human_judgment_traceable` cubre waivers. |
| 6 | La aceptacion IPC atraviesa search, URI, contexto, VFS y timeline; `search_world_result_opens_the_exact_source_uri` y los benchmarks de recuperacion verifican fuentes y presupuesto. |
| 7 | `query_streams_citations_and_offers_proposal_action_for_write_requests`, `proposal_generates_a_ready_draft_for_small_requests` y `ai_run_requires_fresh_final_critique_before_commit_and_persists_summary` ejecutan los contratos offline de consulta, generador, validadores, critico, humano y critica final. |
| 8 | La aceptacion IPC prueba accept, edit y bloqueo stale/revalidacion; el escenario de la mina ejecuta accept/edit/reject y `rejecting_a_required_operation_marks_the_dependency_as_broken` verifica dependencias. |
| 9 | `complete_manual_workflow_covers_replacement_stale_commit_rollback_audit_and_undo` exige juicio y DecisionPoint para replacement; los casos 23-25 de regresion distinguen additive, reinterpretive y replacement. |
| 10 | `proposal_replaces_an_invalid_draft_with_one_complete_repair`, `two_parsing_failures_stop_after_the_single_repair` y `propose_request_omits_write_capabilities` verifican el limite y la ausencia de capacidad de commit. |
| 11 | Las pruebas `constraint_failure_rolls_back_canon_head_and_audit_then_allows_a_corrected_retry`, `real_sqlite_lock_rolls_back_and_commit_succeeds_after_the_lock_is_released`, `simulated_derived_index_failure_rolls_back_canon_head_and_audit_then_retries` y el escenario de la mina cubren constraints, recuperacion y rollback. |
| 12 | La aceptacion comprueba que la credencial solo en memoria no entra al proyecto; `invalid_http_errors_do_not_expose_api_keys_or_lore_bodies`, `two_parsing_failures_stop_after_the_single_repair` y `safety-check.test.mjs` cubren errores, salida, logs y Markdown hostil. |

## Comandos verificados

Ejecutados desde la raiz del repositorio:

```powershell
npm ci --prefix apps\nirmata-desktop\frontend
npm run build --prefix apps\nirmata-desktop\frontend
cargo nextest run --workspace
cargo build -p nirmata-desktop
node --test apps/nirmata-desktop/frontend/safety-check.test.mjs
```

Resultados:

- instalacion frontend: 1 paquete instalado, 0 vulnerabilidades;
- build frontend: completado;
- nextest: 171 ejecutados, 171 aprobados, 1 smoke test de red ignorado;
- build desktop: completado;
- seguridad frontend: 4 ejecutados, 4 aprobados.

No se ejecutaron llamadas de red ni se inspeccionaron o mostraron valores de
`.env`.
