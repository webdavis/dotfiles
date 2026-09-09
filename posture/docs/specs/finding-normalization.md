# Finding normalization policy

Given a query name, the domain admits exactly the 26 captured detectors, both bare and with one nonempty
`pack_<pack>_` prefix removed. It refuses unknown names and the heartbeat canary. The admitted names and
their observed records are in [S025-S360](../acceptance/S025-S360.md).

Given an admitted detector, a numeric-zero fact and a decoded target path, the domain suppresses any row
whose target contains `/.renameio-TempDir`. A zero counter suppresses a baseline row except for
`filevault_off`, `remote_access_sharing_state`, and `agent_exposure_changed`. Other counters survive.

Given typed enrichment paths, the domain selects the field for that detector and changes each tab or
newline to one space. A missing bundle path falls back to `path`; an empty bundle path stays empty. The
original paths are borrowed and remain unchanged. Unenriched detectors return an empty path.

## Codec contracts

The results-log adapter in plan row 3.1 owns JSON (JavaScript Object Notation) parsing, action decoding,
and serialization. Its retained contract emits `q`, `act`, `cols`, and `ep`, in that order, on one line.
Missing, null and false actions default to `changed`; other actions survive. Missing or null columns emit
an empty object. A snapshot array remains one row. Malformed lines disappear independently. Numeric
decoding must distinguish zero from tiny nonzero values without float rounding.

S022 requires an interrupted normalization stage to fail after any prefix output, so its caller does not
advance the cursor. [The captured failure](../acceptance/S022.md) records that obligation before the
adapter is implemented. All existing Bash normalizer tests remain active until caller cutover.

## Preserved test names

The twelve normalizer tests in `test/unit/osquery-normalize-and-digest-store.bats` map below. Rust names
are in `finding::tests`; a codec-owned case keeps its original Bash test until row 3.1 replaces it.
Digest-store tests stay with their existing owner.

| Existing Bash test                                                                                                 | Successor or owner                                                                                       |
| ------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| a packed row reaches the routing stage under its bare query name, with its columns and action intact               | `all_26_detectors_are_admitted_bare_and_packed`; columns/action preservation stays with the codec        |
| only the pack segment is stripped, so a hyphenated pack name leaves the query's own underscores alone              | `only_one_nonempty_pack_segment_is_stripped`                                                             |
| a row that omits its action is normalized to changed, so no later stage special-cases a null                       | Retained Bash contract; action decoding/default belongs to the results-log adapter                       |
| a snapshot-action row stays one finding instead of fanning out its snapshot array                                  | Retained Bash contract; snapshot row decoding belongs to the results-log adapter                         |
| a malformed line drops out without taking the rest of the batch with it                                            | Retained Bash contract; malformed-line omission belongs to the results-log adapter                       |
| an unrecognized query name never becomes a finding, whether it arrives packed or top-level                         | `unknown_names_and_the_heartbeat_canary_are_refused`                                                     |
| the heartbeat canary is dropped defensively, so a stray liveness row can never generate noise                      | `unknown_names_and_the_heartbeat_canary_are_refused`                                                     |
| renameio atomic-write churn is dropped while a real file event on the same query survives                          | `renameio_churn_is_discarded_on_every_admitted_detector`                                                 |
| a counter==0 membership baseline is discarded while counter>0 and counter-absent rows survive                      | `membership_baselines_are_discarded_but_nonbaseline_rows_survive`; numeric decoding stays with the codec |
| the three absolute-state queries keep their counter==0 row, so an already-unsafe state pages on first observation  | `the_three_absolute_state_detectors_keep_their_baselines`                                                |
| the enrich path names the exact file each query type hands the enricher, and is empty where signing does not apply | `the_enrich_path_names_the_file_each_detector_hands_the_enricher`                                        |
| a tab inside a path is squashed to a space, so the enrich path stays one renderable token                          | `tabs_and_newlines_become_individual_spaces_in_the_enrich_path`                                          |
