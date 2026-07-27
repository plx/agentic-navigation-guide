# Show available project commands instead of querying GitHub implicitly.
default:
    @just --list

# Print the next production-readiness issue URL, completion, or waiting state.
get-next-production-readiness-issue *args:
    @python3 scripts/get_next_production_readiness_issue.py {{ args }}

# Run the offline production-readiness selector unit tests.
test-production-readiness-selector:
    PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests -p 'test_get_next_production_readiness_issue.py' -v

# Assert the prepared version against Cargo, source, CLI, changelog, and tag input.
check-release-identity tag="v0.2.0":
    python3 scripts/check_release_identity.py --tag {{ tag }}

# Run fail-closed release-identity checker regressions without building.
test-release-identity:
    PYTHONDONTWRITEBYTECODE=1 python3 -m unittest tests/test_check_release_identity.py -v

# Run fail-closed coverage, mutation-report, and performance policy regressions.
test-quality-gates:
    PYTHONDONTWRITEBYTECODE=1 python3 -m unittest tests/test_check_coverage.py tests/test_check_mutation_report.py tests/test_performance_baseline.py -v

# Reproduce the fixed release performance matrix against its reviewed baseline.
performance-baseline:
    cargo build --release --locked
    python3 scripts/run_performance_baseline.py --reference benchmarks/issue-59-baseline.json
