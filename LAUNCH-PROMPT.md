# GhostMesh — Launch Prompts

## Фазы и зависимости

```
Phase 1: Scaffold ──→ Phase 2: Crypto ──→ Phase 3: Store ──→ Phase 4: Net ──→ Phase 5: Sync
                                                                                    │
                          Phase 8: CLI ←─────────────────────────────────────────────┤
                                                                                    │
                                              Phase 6: Tauri ──→ Phase 7: Frontend ─┤
                                                                                    │
                                                                        Phase 9: Integration
```

| Phase | Scope | Pipeline | Spec §§ |
|---|---|---|---|
| 1. Scaffold | Структура, конфиги, типы | PREFLIGHT → scaffold → TEST → COMMIT | §3, §12, §13 |
| 2. Crypto | Все криптопримитивы | Full | §4, §10 |
| 3. Store | SQLite хранение | Full | §7 |
| 4. Net | Транспорт, wire, discovery | Full | §8 |
| 5. Sync | Lamport, frontier, engine | Full | §6, §9 |
| 6. Tauri | Commands, events, state | Full | §3.3, §3.4, §14 |
| 7. Frontend | Svelte UI, все экраны | Full | §3.5, §11 |
| 8. CLI | Dev CLI с clap | Lightweight | §16.1 |
| 9. Integration | Build, тесты, smoke test | PREFLIGHT → TEST → VERIFY → COMMIT | All |

---

## Запуск одной фазы

```bash
claude --dangerously-skip-permissions
```

Затем:

```
Phase 2: Crypto
```

Claude прочитает CLAUDE.md, найдёт определение Phase 2, запустит полный pipeline только для крипто-модуля.

---

## Запуск нескольких фаз подряд

```
Phases 2-5
```

Выполнит фазы 2, 3, 4, 5 последовательно. Коммит после каждой фазы. Engram передаёт контекст между фазами.

---

## Запуск всего проекта

```
Phases 1-9
```

Полная реализация за 9 прогонов pipeline. Каждая фаза — отдельный коммит.

---

## Продолжение после обрыва

```
Continue
```

Claude определит текущее состояние из git + Engram и продолжит с ближайшей незавершённой фазы.

---

## Headless запуск (без интерактива)

```bash
# Одна фаза
claude --dangerously-skip-permissions -p \
  "Прочитай CLAUDE.md. Выполни Phase 2: Crypto. Autonomous mode, auto-commit."

# Диапазон фаз
claude --dangerously-skip-permissions -p \
  "Прочитай CLAUDE.md. Выполни Phases 1-5 последовательно. Autonomous mode, auto-commit после каждой фазы."

# Продолжение
claude --dangerously-skip-permissions -p \
  "Прочитай CLAUDE.md. Continue — определи текущее состояние из git + Engram, продолжи с незавершённой фазы."
```

---

## Что происходит при запуске фазы

1. **PREFLIGHT** — Team Lead проверяет git status, entry criteria, ищет в Engram
2. **READ** — Planner читает нужные секции спеки и существующий код
3. **PLAN** — Planner создаёт план work units для фазы
4. **MEMORY_CHECK** — Memory Checker проверяет план против Engram
5. **PLAN_REVIEW** — Plan Reviewer проверяет полноту vs спека
6. **CODER** — Coder реализует work units (test-first)
7. **MEMORY_CHECK** — Memory Checker проверяет код
8. **REVIEW ×3** — Security + Quality + Coverage параллельно
9. **TEST** — Tester запускает тесты фазы
10. **VERIFY** — Verifier проверяет vs спека
11. **COMMIT** — Team Lead коммитит: `phase-N: description`

Каждый агент: `memory_search` → работа → `memory_store` → `memory_judge`.

---

## Если что-то пошло не так

| Ситуация | Действие |
|---|---|
| Pipeline застрял | Переключись на агента (Shift+Down), прочитай статус, дай инструкцию |
| Engram недоступен | Агенты продолжат без памяти, batch-store позже |
| Build fails | Tester сохранит ошибку, pipeline вернётся к Coder (max 3 итерации) |
| Entry criteria не выполнены | Pipeline остановится — сначала заверши предыдущую фазу |
| Токены кончились | Промежуточное состояние в git + Engram. `Continue` в новой сессии |
| Нужно переделать фазу | `Phase N` запустит фазу заново поверх существующего кода |
