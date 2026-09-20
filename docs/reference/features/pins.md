# Pins

Select an item and use **Pin to All** or **Pin to Apps**, or the item's category, in the detail panel or Actions menu. Each category has its own pin list. You can pin the same item to both All and its category. Removing one pin keeps the other pin. Pinned items appear in a Pinned section when that category has an empty query. Typed searches keep their match order. Pins stay saved in the local SQLite database after a restart. Existing app pins move to both Apps and All when the database updates.

Tool pins save their input and output choice. When you open a category, TinyDash uses this input to show current tool results. Password pins save the generator type and length. They generate a new value after a restart. Generated passwords are not stored as pins.

## Verification

Pin an item to All and its own category. Remove one pin and confirm the other remains. Check the empty-query list and a typed search separately.

Tests: [tests/pins.spec.ts](../../../tests/pins.spec.ts).

Browser tests verify UI requests. Rust database tests verify persistence and migration behavior.

Use the [verification procedure](../../how-to/verify.md) and [desktop checks](../../how-to/desktop-checks.md).
