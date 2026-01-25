#!/usr/bin/env bats

# Integration tests for the ingest binary
# Tests rule ingestion, database population, and file storage

setup() {
    # Create clean test environment
    export TEST_DIR="$(mktemp -d)"
    export ORIG_DIR="$(pwd)"
    cd "$TEST_DIR"

    # Build the ingest binary if not already built
    if [ ! -f "$ORIG_DIR/target/debug/ingest" ]; then
        cd "$ORIG_DIR"
        cargo build --bin ingest
        cd "$TEST_DIR"
    fi

    export INGEST_BIN="$ORIG_DIR/target/debug/ingest"

    # Create necessary directories
    mkdir -p data etc/contexts test/fixtures

    # Create a default context file
    cat > etc/contexts/default.json <<EOF
{
  "jurisdiction": "US-CA",
  "tz": "America/Los_Angeles"
}
EOF
}

teardown() {
    cd "$ORIG_DIR"
    rm -rf "$TEST_DIR"
}

# Helper function to create a basic rule file for common tests
create_basic_rule() {
    local filename="$1"
    local rule_id="${2:-test-rule-001}"

    cat > "$filename" <<EOF
PROPERTIES
ID $rule_id
NAME TestRule
VERSION 1.0

IN EFFECT
IN US-CA, FROM 2024-01-01T00:00, TO 2025-12-31T23:59, TZ America/Los_Angeles

CONDITIONS
age>='18': [01, 01, 00, 00]
citizen='true': [01, 00, 01, 00]

ASSERTIONS
eligible:='true': [01, 00, 00, 00]
status:='pending': [00, 01, 01, 00]
EOF
}

# Helper function to query SQLite database
query_db() {
    sqlite3 data/rules.db "$1"
}

# Helper function to count rows in a table
count_rows() {
    local table="$1"
    query_db "SELECT COUNT(*) FROM $table;"
}

# Test 1: Basic ingestion creates database
@test "ingest creates rules.db on first run" {
    create_basic_rule "test_rule.rule"

    [ ! -f data/rules.db ]

    run "$INGEST_BIN" test_rule.rule
    [ "$status" -eq 0 ]
    [ -f data/rules.db ]
}

# Test 2: Database schema is created correctly
@test "ingest creates correct database schema" {
    create_basic_rule "test_rule.rule"
    "$INGEST_BIN" test_rule.rule

    # Check in_effect table exists
    run query_db "SELECT name FROM sqlite_master WHERE type='table' AND name='in_effect';"
    [ "$status" -eq 0 ]
    [[ "$output" == "in_effect" ]]

    # Check applicable table exists
    run query_db "SELECT name FROM sqlite_master WHERE type='table' AND name='applicable';"
    [ "$status" -eq 0 ]
    [[ "$output" == "applicable" ]]
}

# Test 3: Rule file is stored in correct location
@test "ingest stores rule file with correct path structure" {
    create_basic_rule "test_rule.rule" "my-rule-id"

    run "$INGEST_BIN" test_rule.rule
    [ "$status" -eq 0 ]

    # Check file exists at data/rules/my-rule-id/0.rule
    [ -f data/rules/my-rule-id/0.rule ]
}

# Test 4: in_effect table is populated
@test "ingest populates in_effect table correctly" {
    create_basic_rule "test_rule.rule" "rule-123"
    "$INGEST_BIN" test_rule.rule

    # Check row count
    result=$(count_rows "in_effect")
    [ "$result" -eq 1 ]

    # Check content
    run query_db "SELECT rule_id, jurisdiction, tz FROM in_effect WHERE rule_id='rule-123';"
    [ "$status" -eq 0 ]
    [[ "$output" == *"rule-123"* ]]
    [[ "$output" == *"US-CA"* ]]
    [[ "$output" == *"America/Los_Angeles"* ]]
}

# Test 5: applicable table is populated with keys
@test "ingest extracts and stores condition keys" {
    create_basic_rule "test_rule.rule" "rule-456"
    "$INGEST_BIN" test_rule.rule

    # Check that keys are extracted
    result=$(count_rows "applicable")
    [ "$result" -eq 2 ]

    # Check for specific keys
    run query_db "SELECT key FROM applicable WHERE rule_id='rule-456' ORDER BY key;"
    [ "$status" -eq 0 ]
    [[ "$output" == *"age"* ]]
    [[ "$output" == *"citizen"* ]]
}



# Test 6: Revision increments on re-ingestion
@test "ingest increments revision number for existing rule" {
    create_basic_rule "test_rule_v1.rule" "versioned-rule"
    "$INGEST_BIN" test_rule_v1.rule

    [ -f data/rules/versioned-rule/0.rule ]

    # Ingest again
    create_basic_rule "test_rule_v2.rule" "versioned-rule"
    "$INGEST_BIN" test_rule_v2.rule

    [ -f data/rules/versioned-rule/0.rule ]
    [ -f data/rules/versioned-rule/1.rule ]
}

# Test 7: Multiple conditions create multiple applicable entries
@test "ingest creates applicable entry for each condition key" {
    cat > multi_cond.rule <<EOF
PROPERTIES
ID multi-cond-rule

IN EFFECT
IN US-CA, FROM 2024-01-01T00:00, TO 2024-12-31T23:59, TZ America/Los_Angeles

CONDITIONS
age>='18': [01, 00]
income>='50000': [01, 00]
employed='true': [01, 00]
resident='true': [01, 00]

ASSERTIONS
eligible:='true': [01, 00]
EOF

    "$INGEST_BIN" multi_cond.rule

    result=$(count_rows "applicable")
    [ "$result" -eq 4 ]

    # Verify all keys are present
    run query_db "SELECT COUNT(DISTINCT key) FROM applicable WHERE rule_id='multi-cond-rule';"
    [ "$output" -eq 4 ]
}

parse_uuid_variant(){
    uuidparse --json "$1" | jq -r '.uuids[].variant'
}

# Test 8: Generated UUID when no ID provided
@test "ingest generates UUID when ID property is missing" {
    cat > no_id.rule <<EOF
PROPERTIES
NAME TestRuleNoID

IN EFFECT
IN US-CA, FROM 2024-01-01T00:00, TO 2024-12-31T23:59, TZ America/Los_Angeles

CONDITIONS
age>='18': [01, 00]

ASSERTIONS
eligible:='true': [01, 00]
EOF

    "$INGEST_BIN" no_id.rule

    # Should have created a directory with UUID name
    rule_count=$(ls -d data/rules/*/ 2>/dev/null | wc -l)
    [ "$rule_count" -eq 1 ]

    # Check that a rule_id exists in the database
    result=$(count_rows "in_effect")
    [ "$result" -eq 1 ]

    variant=$(parse_uuid_variant $(basename data/rules/*))
    [ "$variant" != "invalid" ]
}

# Test 9: Database integrity with multiple rules
@test "ingest maintains database integrity with multiple rules" {
    create_basic_rule "rule1.rule" "rule-001"
    create_basic_rule "rule2.rule" "rule-002"
    create_basic_rule "rule3.rule" "rule-003"

    "$INGEST_BIN" rule1.rule
    "$INGEST_BIN" rule2.rule
    "$INGEST_BIN" rule3.rule

    # Check total rows
    result=$(count_rows "in_effect")
    [ "$result" -eq 3 ]

    result=$(count_rows "applicable")
    [ "$result" -eq 6 ]  # 2 keys per rule * 3 rules

    # Verify distinct rule_ids
    run query_db "SELECT COUNT(DISTINCT rule_id) FROM in_effect;"
    [ "$output" -eq 3 ]
}

# Test 10: Handle rule without conditions
@test "ingest handles rule with no conditions gracefully" {
    cat > no_conditions.rule <<EOF
PROPERTIES
ID no-cond-rule

IN EFFECT
IN US-CA, FROM 2024-01-01T00:00, TO 2024-12-31T23:59, TZ America/Los_Angeles

ASSERTIONS
eligible:='true': [01]
EOF

    run "$INGEST_BIN" no_conditions.rule
    [ "$status" -eq 0 ]

    # Should have in_effect entry
    result=$(query_db "SELECT COUNT(*) FROM in_effect WHERE rule_id='no-cond-rule';")
    [ "$result" -eq 1 ]

    # Should have no applicable entries
    result=$(query_db "SELECT COUNT(*) FROM applicable WHERE rule_id='no-cond-rule';")
    [ "$result" -eq 0 ]
}

# Test 11: Verify stored file content matches input
@test "ingest preserves original rule content in stored file" {
    create_basic_rule "original.rule" "preserve-test"

    "$INGEST_BIN" original.rule

    # Compare original and stored file
    run diff original.rule data/rules/preserve-test/0.rule
    [ "$status" -eq 0 ]
}

# Test 12: Version numbers in database match file structure
@test "ingest version numbers are consistent between DB and filesystem" {
    create_basic_rule "v1.rule" "version-test"
    "$INGEST_BIN" v1.rule

    create_basic_rule "v2.rule" "version-test"
    "$INGEST_BIN" v2.rule

    create_basic_rule "v3.rule" "version-test"
    "$INGEST_BIN" v3.rule

    # Check filesystem has versions 0, 1, 2
    [ -f data/rules/version-test/0.rule ]
    [ -f data/rules/version-test/1.rule ]
    [ -f data/rules/version-test/2.rule ]

    # Check database has corresponding versions
    run query_db "SELECT DISTINCT version FROM in_effect WHERE rule_id='version-test' ORDER BY version;"
    [[ "$output" == *"0"* ]]
    [[ "$output" == *"1"* ]]
    [[ "$output" == *"2"* ]]
}

# Test 13: Datetime parsing in in_effect table
@test "ingest correctly parses and stores datetime values" {
    create_basic_rule "datetime_test.rule" "dt-rule"
    "$INGEST_BIN" datetime_test.rule

    run query_db "SELECT from_t, to_t FROM in_effect WHERE rule_id='dt-rule';"
    [ "$status" -eq 0 ]
    [[ "$output" == *"2024-01-01"* ]]
    [[ "$output" == *"2025-12-31"* ]]
}

# Test 14: Error handling for missing file
@test "ingest fails gracefully when rule file doesn't exist" {
    run "$INGEST_BIN" nonexistent.rule
    [ "$status" -ne 0 ]
}


