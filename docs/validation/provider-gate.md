# Gate del proveedor de IA

## Decision

NIR-087 y NIR-088 quedan cerrados en la implementacion: el cliente concreto
`AzureFoundryCapabilityClient` cubre las capacidades requeridas y no existe una
carencia que justifique otro proveedor, un trait publico, una factory o un
registro de proveedores.

`AiModeClient` permanece `pub(crate)` dentro de `nirmata-app`. Es la frontera
interna que permite probar los casos de uso offline y despacha en produccion al
cliente Azure concreto; no forma una API de marketplace ni se exporta.

## Auditoria

| Capacidad | Implementacion vigente | Evidencia ejecutable |
|---|---|---|
| Query | `AzureFoundryCapabilityClient::query` y `query_streaming`, prompt `query_v1` | `query_rejects_change_set_output`, `query_streaming_aggregates_deltas_and_parses_output` |
| Propose | `propose`, prompt `propose_v2`, salida `ChangeSetDraft` validada | `propose_request_omits_write_capabilities` y regresion de app |
| Critic | `critic`, prompt dedicado `critic_v3` | `critic_uses_a_dedicated_prompt` y regresion semantica de app |
| Specialist | `specialist` y `synthesize`, prompts y presupuestos fijos | `deep_capabilities_use_fixed_prompts_tokens_and_no_tools` y suite `deep_review` |
| Import extraction | `extract_import`, prompt no confiable y DTO con citas hash/chunk | `import_extraction_uses_its_grounded_read_only_prompt` y suite `lore_import` |
| Internal document | `generate_internal_document`, prompt y DTO propios | `internal_document_uses_its_strict_grounded_prompt`, `internal_document_is_strict_and_requires_references` y pruebas de app de perspectiva/revision |
| Streaming | Responses SSE con agregacion y marcador final | `streams_response_successfully`, `query_streaming_aggregates_deltas_and_parses_output`, cancelacion e interrupcion de stream |
| Timeout y cancelacion | `RequestOptions` y `run_with_request_controls` compartidos por todas las llamadas | `times_out_requests`, `cancels_requests_explicitly`, `cancelling_an_active_stream_discards_partial_output_and_allows_retry` |
| DTO estricto | `deny_unknown_fields`, validacion semantica y reconstruccion canonica de drafts | suite `contracts`, incluida la salida de documento interno |
| Credenciales | Windows Credential Manager; fallback explicito de sesion; status sin secreto | `credential_store_sets_reads_and_clears_keys`, `unavailable_secure_store_falls_back_to_session_memory_with_limitation` |
| Seguridad de errores | redaccion de API key y descarte de cuerpos de error del proveedor | `invalid_http_errors_do_not_expose_api_keys_or_lore_bodies` |
| Privacidad | `response_request_body` fuerza `"store": false` para streaming y no streaming | `creates_response_successfully`, `propose_request_omits_write_capabilities`, prueba de import extraction |
| Frontera concreta | un solo cliente publico Azure; sin provider trait/factory publica | `provider_boundary_stays_concrete_without_marketplace_abstraction` |

Todas las capacidades usan el mismo endpoint Azure Responses y los mismos
controles de transporte. Los especialistas son configuraciones del pipeline, no
proveedores adicionales. Las pruebas son offline; el unico smoke test de red
permanece ignorado salvo ejecucion explicita con credenciales.

## Comandos del gate

```powershell
cargo test -p nirmata-ai capabilities
cargo test -p nirmata-ai contracts
cargo test -p nirmata-ai runtime
cargo test -p nirmata-app ai
cargo test -p nirmata-app deep_review
cargo test -p nirmata-app lore_import
```

El 12 de agosto de 2026, capabilities pasó 8/8, contracts 7/7, runtime 12/12
con 1 smoke de red omitido, y la suite global pasó 256/256 pruebas offline.
