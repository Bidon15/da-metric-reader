# Sampling Logic - Forgiving but Accurate

## The Problem You Identified

```
13:28:33 - Metrics arrive (Head: 8549695)
13:28:34 - Sample checks (1 second later!)
           Result: "head not advanced" ❌ FALSE NEGATIVE!
13:28:43 - New metrics arrive (Head: 8549697)
```

**Issue**: The sampler was too strict - it checked if the head advanced in exactly 30 seconds, but metrics arrive on their own schedule!

## The Solution: Grace Period Window

### New Logic

Instead of just checking "did head advance?", we now ask:

1. **Did the head advance?** → ✅ PASS (clearly healthy)
2. **Head didn't advance, BUT data is fresh (< 45 seconds old)?** → ✅ PASS (timing issue, not a real problem)
3. **Head didn't advance AND data is older (> 45 seconds)?** → ❌ FAIL (real issue)

**Why 45 seconds?** With 6-second Celestia blocks, 45 seconds = ~7-8 blocks. This gives plenty of time for the head to advance before declaring it stuck.

### Code Behavior

```rust
if head_advanced_by >= 1 {
    ✅ PASS: "ok (+2 blocks)"
} else if data_age <= 45 seconds {
    ✅ PASS: "ok (fresh data, age=20s)"
} else {
    ❌ FAIL: "head stuck at 8549695"
}
```

## Example Scenarios

### Scenario 1: Normal Operation

```
13:00:00 - Metrics arrive (Head: 100)
13:00:30 - Sample checks
           - prev_head: 98, curr_head: 100
           - Result: ✅ "ok (+2 blocks)"
```

### Scenario 2: Unfortunate Timing (NOW HANDLED!)

```
13:00:29 - Metrics arrive (Head: 100)
13:00:30 - Sample checks (1 second later)
           - prev_head: 100, curr_head: 100
           - Data age: 1 second
           - Result: ✅ "ok (fresh data, age=1s)"
```

### Scenario 3: Actually Stuck

```
12:59:40 - Metrics arrive (Head: 100)
13:00:30 - Sample checks (50 seconds later)
           - prev_head: 100, curr_head: 100
           - Data age: 50 seconds (> 45s grace period)
           - Result: ❌ "head stuck at 100"
```

### Scenario 4: Stale Metrics

```
12:58:00 - Last metrics (Head: 100)
13:00:30 - Sample checks (150 seconds later)
           - Data age: 150 seconds > 120s threshold
           - Result: ❌ "stale (age > 120s)"
```

## Updated Output

### Before (Too Strict)

```
❌ Sample FAILED - head not advanced (prev: Some(8549695), curr: Some(8549695))
```

### After (Forgiving)

```
✅ Sample OK - Head: Some(8549695) (fresh data, age=1s) | Buffer: 45/120 samples
```

Or when actually advancing:

```
✅ Sample OK - Head: Some(8549697) (+2 blocks) | Buffer: 46/120 samples
```

## Configuration

The grace period is configurable in `config.toml`:

```toml
[sampling]
tick_secs = 30
window_secs = 3600
max_staleness_secs = 120
grace_period_secs = 45  # Allow up to 45s for head to advance (~7-8 Celestia blocks)
```

This creates a three-tier system:

```
Metrics age:
├─ 0-45s:   Grace period - give benefit of doubt ✅
├─ 45-120s: Must see progress - expect advancement ⚠️
└─ >120s:   Stale data - mark as failed ❌
```

**Tuning the grace period:**

- **Too short** (< 30s): Risk of false negatives from timing issues
- **Too long** (> 60s): May miss genuine problems
- **Sweet spot** (30-45s): Balances forgiveness with responsiveness

For Celestia with 6s blocks:

- 30s = ~5 blocks
- 45s = ~7-8 blocks ✅ (recommended)
- 60s = ~10 blocks

## Why This Matters for Your Goals

### Goal 1: Prove uptime without being harsh ✅

**Before**: Penalized nodes for timing misalignment
**After**: Only penalizes actual downtime

### Goal 2: Realistic crypto uptime tracking ✅

This approach:

- ✅ Catches real issues (stuck head > 10s)
- ✅ Ignores timing artifacts (fresh data)
- ✅ Requires continuous operation (stale = fail)
- ✅ Aligns with blockchain reality (every block matters)

## Real-World Example

Your node pushing metrics every 15 seconds:

```
Timeline:
00:00 - Metrics (Head: 100)
00:15 - Metrics (Head: 102) ← pushed
00:16 - Sample checks → ✅ ok (fresh data, age=1s)
00:30 - Metrics (Head: 104) ← pushed
00:46 - Sample checks → ✅ ok (+2 blocks)
01:00 - Metrics (Head: 106) ← pushed
01:01 - Metrics (Head: 108) ← pushed
01:16 - Sample checks → ✅ ok (+2 blocks)
```

**Result**: All samples pass, even though timing isn't perfectly aligned!

## When Samples Still Fail

Samples will only fail if there's a **real problem**:

1. **Stale metrics** (> 120s old) - Node stopped reporting
2. **Head stuck** (same value for > 10s) - Node not progressing
3. **Headers not advancing** - Sampling isn't working
4. **No data** - Metrics never arrived

## Summary

The sampler is now **forgiving but accurate**:

- Tolerates timing misalignment (10-second grace period)
- Still catches real issues (stuck head, stale data)
- Better reflects actual node health
- Reduces false negatives significantly

This gives you **realistic uptime metrics** without harsh penalties for things that aren't real problems! 🎯
