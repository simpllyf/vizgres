# Vizgres Project Memory

## 1.0 Release Critical Issues (Priority Order)

1. **UTF-8 cursor handling** - CRITICAL - Editor uses `.len()` (bytes) instead of `.chars().count()`. Breaks with emoji/accents.

2. **Query timeout** - HIGH - No timeout config. Hung queries block UI forever. Need configurable timeout (default 30s).

3. **Connection loss handling** - HIGH - Silent failure. App continues with stale connection. Need reconnect dialog.

4. **Panics in event handling** - MEDIUM - 5 `panic!()` calls in production paths (connection_dialog.rs:636, 830). Replace with error handling.

5. **Large result OOM protection** - MEDIUM - No row limits. Loading millions of rows crashes. Need configurable limit (default 1000) with "load more" option.
