# Contributing

Thank you for helping. Issues and pull requests are welcome on
https://github.com/SotharL/CargoFrame.

- Before a pull request, run the local checks: `CARGO_INCREMENTAL=0 scripts/dev/check.sh all`
  (see [the development plan](../docs/development-plan.md#9-quality-tests-gates-budgets)).
- Every text people read goes through `t()` and is translated into French, Spanish, Italian and
  German (`node scripts/i18n/check.mjs --missing` lists what is left).
- What the app is and how it works: [docs/design.md](../docs/design.md); why choices were made:
  [docs/decisions.md](../docs/decisions.md).

## License of contributions

Unless you say otherwise, any contribution you send for inclusion in CargoFrame is licensed under
the MIT License or the Apache License 2.0, at the choice of whoever uses it, as CargoFrame itself
is, without any additional terms or conditions.
