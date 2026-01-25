#!/usr/bin/env bats

# Tests for IOR format rule parsing and ingestion
# Focuses on the custom section-based rule format

setup() {
    export TEST_DIR="$(mktemp -d)"
    export ORIG_DIR="$(pwd)"
    cd "$TEST_DIR"

    if [ ! -f "$ORIG_DIR/target/debug/ingest" ]; then
        cd "$ORIG_DIR"
        cargo build --bin ingest
        cd "$TEST_DIR"
    fi

    export INGEST_BIN="$ORIG_DIR/target/debug/ingest"
    mkdir -p data etc/contexts

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

create_basic_ior_rule() {
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

@test "ior format basic rule ingestion" {
    create_basic_ior_rule "test.rule" "ior-basic"

    run "$INGEST_BIN" test.rule
    [ "$status" -eq 0 ]
    [ -f data/rules/ior-basic/0.rule ]
}

@test "ior format parses all sections correctly" {
    cat > full_sections.rule <<EOF
PROPERTIES
ID full-section-rule
NAME CompleteRule
VERSION 2.1
AUTHOR TestAuthor

IN EFFECT
IN US-CA, FROM 2024-01-01T00:00, TO 2024-12-31T23:59, TZ America/Los_Angeles

CONDITIONS
age>='18': [01, 01, 00, 00]
income<='100000': [01, 00, 01, 00]
resident='true': [01, 00, 00, 01]

ASSERTIONS
eligible:='true': [01, 00, 00, 00]
priority:='high': [00, 01, 01, 00]
status:='pending': [00, 00, 01, 01]
EOF

    run "$INGEST_BIN" full_sections.rule
    [ "$status" -eq 0 ]

    # Check all condition keys are extracted
    result=$(sqlite3 data/rules.db "SELECT COUNT(*) FROM applicable WHERE rule_id='full-section-rule';")
    [ "$result" -eq 3 ]
}

@test "ior format handles multiple in_effect clauses" {
    cat > multi_effect.rule <<EOF
PROPERTIES
ID multi-effect-ior

IN EFFECT
IN US-CA, FROM 2024-01-01T00:00, TO 2024-12-31T23:59, TZ America/Los_Angeles
IN US-NY, FROM 2024-06-01T00:00, TO 2024-12-31T23:59, TZ America/New_York

CONDITIONS
age>='21': [01, 00]

ASSERTIONS
eligible:='true': [01, 00]
EOF

    run "$INGEST_BIN" multi_effect.rule
    [ "$status" -eq 0 ]

    result=$(sqlite3 data/rules.db "SELECT COUNT(*) FROM in_effect WHERE rule_id='multi-effect-ior';")
    [ "$result" -eq 2 ]
}

@test "ior format handles various operators in conditions" {
    cat > operators.rule <<EOF
PROPERTIES
ID operators-ior

IN EFFECT
IN US-CA, FROM 2024-01-01T00:00, TO 2024-12-31T23:59, TZ America/Los_Angeles

CONDITIONS
age>='18': [01, 00]
score<='100': [01, 00]
name='John': [01, 00]
active!='false': [01, 00]
rating>='4.5': [01, 00]

ASSERTIONS
result:='pass': [01, 00]
EOF

    run "$INGEST_BIN" operators.rule
    [ "$status" -eq 0 ]

    result=$(sqlite3 data/rules.db "SELECT COUNT(*) FROM applicable WHERE rule_id='operators-ior';")
    [ "$result" -eq 5 ]
}

@test "ior format handles rule without conditions" {
    cat > no_conditions.rule <<EOF
PROPERTIES
ID no-conditions-ior
NAME SimpleRule

IN EFFECT
IN US-CA, FROM 2024-01-01T00:00, TO 2024-12-31T23:59, TZ America/Los_Angeles

ASSERTIONS
eligible:='true': [01]
EOF

    run "$INGEST_BIN" no_conditions.rule
    [ "$status" -eq 0 ]

    # Should have in_effect entry
    result=$(sqlite3 data/rules.db "SELECT COUNT(*) FROM in_effect WHERE rule_id='no-conditions-ior';")
    [ "$result" -eq 1 ]

    # Should have no applicable entries
    result=$(sqlite3 data/rules.db "SELECT COUNT(*) FROM applicable WHERE rule_id='no-conditions-ior';")
    [ "$result" -eq 0 ]
}

@test "ior format preserves original content" {
    create_basic_ior_rule "original.rule" "preserve-ior"

    "$INGEST_BIN" original.rule

    run diff original.rule data/rules/preserve-ior/0.rule
    [ "$status" -eq 0 ]
}

@test "ior format handles comments and empty lines" {
    cat > comments.rule <<EOF
# This is a comment
PROPERTIES
ID comments-ior
NAME RuleWithComments

# Empty line above and below

IN EFFECT
IN US-CA, FROM 2024-01-01T00:00, TO 2024-12-31T23:59, TZ America/Los_Angeles

# Another comment
CONDITIONS
age>='18': [01, 00]  # Inline comment
citizen='true': [01, 00]

ASSERTIONS
eligible:='true': [01, 00]
EOF

    run "$INGEST_BIN" comments.rule
    [ "$status" -eq 0 ]
}

@test "ior format generates UUID when no ID provided" {
    cat > no_id.rule <<EOF
PROPERTIES
NAME NoIDRule
VERSION 1.0

IN EFFECT
IN US-CA, FROM 2024-01-01T00:00, TO 2024-12-31T23:59, TZ America/Los_Angeles

CONDITIONS
age>='18': [01, 00]

ASSERTIONS
eligible:='true': [01, 00]
EOF

    run "$INGEST_BIN" no_id.rule
    [ "$status" -eq 0 ]

    # Should have created a directory with UUID name
    rule_count=$(ls -d data/rules/*/ 2>/dev/null | wc -l)
    [ "$rule_count" -eq 1 ]
}

@test "ior format handles complex case patterns" {
    cat > complex_cases.rule <<EOF
PROPERTIES
ID complex-cases-ior

IN EFFECT
IN US-CA, FROM 2024-01-01T00:00, TO 2024-12-31T23:59, TZ America/Los_Angeles

CONDITIONS
age>='18': [01, 01, 00, 00]
income>='50000': [01, 00, 01, 00]
credit>='700': [01, 00, 00, 01]

ASSERTIONS
eligible:='true': [01, 00, 00, 00]
priority:='high': [00, 01, 01, 00]
review:='needed': [00, 00, 01, 01]
EOF

    run "$INGEST_BIN" complex_cases.rule
    [ "$status" -eq 0 ]

    # Should extract 3 condition keys
    result=$(sqlite3 data/rules.db "SELECT COUNT(*) FROM applicable WHERE rule_id='complex-cases-ior';")
    [ "$result" -eq 3 ]
}