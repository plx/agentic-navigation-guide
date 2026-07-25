# Show available project commands instead of querying GitHub implicitly.
default:
    @just --list

# Print the next production-readiness issue URL, completion, or waiting state.
get-next-production-readiness-issue *args:
    @python3 scripts/get_next_production_readiness_issue.py {{ args }}

# Run the offline production-readiness selector unit tests.
test-production-readiness-selector:
    PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests -p 'test_get_next_production_readiness_issue.py' -v
