#!/usr/bin/env bats

# Tests for JSON format rule parsing and ingestion
# Focuses on JSON structured rule format

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

create_basic_json_rule() {
    local filename="$1"
    local rule_id="${2:-test-rule-001}"

    cat > "$filename" <<EOF
{
  "properties": {
    "id": "$rule_id",
    "name": "TestRule",
    "version": "1.0"
  },
  "in_effect": [
    {
      "in": "US-CA",
      "from": "2024-01-01T00:00",
      "to": "2025-12-31T23:59",
      "tz": "America/Los_Angeles"
    }
  ],
  "input_conditions": [
    {
      "expression": {
        "key": "age",
        "operator": ">=",
        "value": "18"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "01"},
        {"case": "00"},
        {"case": "00"}
      ]
    },
    {
      "expression": {
        "key": "citizen",
        "operator": "=",
        "value": "true"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "00"},
        {"case": "01"},
        {"case": "00"}
      ]
    }
  ],
  "output_assertions": [
    {
      "key": "eligible",
      "value": "true",
      "scenarios": [
        {"case": "01"},
        {"case": "00"},
        {"case": "00"},
        {"case": "00"}
      ]
    },
    {
      "key": "status",
      "value": "pending",
      "scenarios": [
        {"case": "00"},
        {"case": "01"},
        {"case": "01"},
        {"case": "00"}
      ]
    }
  ]
}
EOF
}

@test "json format basic rule ingestion" {
    create_basic_json_rule "test.json" "json-basic"

    run "$INGEST_BIN" test.json
    [ "$status" -eq 0 ]
    [ -f data/rules/json-basic/0.json ]
}

@test "json format parses all sections correctly" {
    cat > full_sections.json <<EOF
{
  "properties": {
    "id": "full-section-json",
    "name": "CompleteRule",
    "version": "2.1",
    "author": "TestAuthor"
  },
  "in_effect": [
    {
      "in": "US-CA",
      "from": "2024-01-01T00:00",
      "to": "2024-12-31T23:59",
      "tz": "America/Los_Angeles"
    }
  ],
  "input_conditions": [
    {
      "expression": {
        "key": "age",
        "operator": ">=",
        "value": "18"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "01"},
        {"case": "00"},
        {"case": "00"}
      ]
    },
    {
      "expression": {
        "key": "income",
        "operator": "<=",
        "value": "100000"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "00"},
        {"case": "01"},
        {"case": "00"}
      ]
    },
    {
      "expression": {
        "key": "resident",
        "operator": "=",
        "value": "true"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "00"},
        {"case": "00"},
        {"case": "01"}
      ]
    }
  ],
  "output_assertions": [
    {
      "key": "eligible",
      "value": "true",
      "scenarios": [
        {"case": "01"},
        {"case": "00"},
        {"case": "00"},
        {"case": "00"}
      ]
    },
    {
      "key": "priority",
      "value": "high",
      "scenarios": [
        {"case": "00"},
        {"case": "01"},
        {"case": "01"},
        {"case": "00"}
      ]
    },
    {
      "key": "status",
      "value": "pending",
      "scenarios": [
        {"case": "00"},
        {"case": "00"},
        {"case": "01"},
        {"case": "01"}
      ]
    }
  ]
}
EOF

    run "$INGEST_BIN" full_sections.json
    [ "$status" -eq 0 ]

    # Check all condition keys are extracted
    result=$(sqlite3 data/rules.db "SELECT COUNT(*) FROM applicable WHERE rule_id='full-section-json';")
    [ "$result" -eq 3 ]
}

@test "json format handles multiple in_effect entries" {
    cat > multi_effect.json <<EOF
{
  "properties": {
    "id": "multi-effect-json"
  },
  "in_effect": [
    {
      "in": "US-CA",
      "from": "2024-01-01T00:00",
      "to": "2024-12-31T23:59",
      "tz": "America/Los_Angeles"
    },
    {
      "in": "US-NY",
      "from": "2024-06-01T00:00",
      "to": "2024-12-31T23:59",
      "tz": "America/New_York"
    }
  ],
  "input_conditions": [
    {
      "expression": {
        "key": "age",
        "operator": ">=",
        "value": "21"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "00"}
      ]
    }
  ],
  "output_assertions": [
    {
      "key": "eligible",
      "value": "true",
      "scenarios": [
        {"case": "01"},
        {"case": "00"}
      ]
    }
  ]
}
EOF

    run "$INGEST_BIN" multi_effect.json
    [ "$status" -eq 0 ]

    result=$(sqlite3 data/rules.db "SELECT COUNT(*) FROM in_effect WHERE rule_id='multi-effect-json';")
    [ "$result" -eq 2 ]
}

@test "json format handles various operators in conditions" {
    cat > operators.json <<EOF
{
  "properties": {
    "id": "operators-json"
  },
  "in_effect": [
    {
      "in": "US-CA",
      "from": "2024-01-01T00:00",
      "to": "2024-12-31T23:59",
      "tz": "America/Los_Angeles"
    }
  ],
  "input_conditions": [
    {
      "expression": {
        "key": "age",
        "operator": ">=",
        "value": "18"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "00"}
      ]
    },
    {
      "expression": {
        "key": "score",
        "operator": "<=",
        "value": "100"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "00"}
      ]
    },
    {
      "expression": {
        "key": "name",
        "operator": "=",
        "value": "John"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "00"}
      ]
    },
    {
      "expression": {
        "key": "active",
        "operator": "!=",
        "value": "false"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "00"}
      ]
    },
    {
      "expression": {
        "key": "rating",
        "operator": ">=",
        "value": "4.5"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "00"}
      ]
    }
  ],
  "output_assertions": [
    {
      "key": "result",
      "value": "pass",
      "scenarios": [
        {"case": "01"},
        {"case": "00"}
      ]
    }
  ]
}
EOF

    run "$INGEST_BIN" operators.json
    [ "$status" -eq 0 ]

    result=$(sqlite3 data/rules.db "SELECT COUNT(*) FROM applicable WHERE rule_id='operators-json';")
    [ "$result" -eq 5 ]
}

@test "json format handles rule without conditions" {
    cat > no_conditions.json <<EOF
{
  "properties": {
    "id": "no-conditions-json",
    "name": "SimpleRule"
  },
  "in_effect": [
    {
      "in": "US-CA",
      "from": "2024-01-01T00:00",
      "to": "2024-12-31T23:59",
      "tz": "America/Los_Angeles"
    }
  ],
  "output_assertions": [
    {
      "key": "eligible",
      "value": "true",
      "scenarios": [
        {"case": "01"}
      ]
    }
  ]
}
EOF

    run "$INGEST_BIN" no_conditions.json
    [ "$status" -eq 0 ]

    # Should have in_effect entry
    result=$(sqlite3 data/rules.db "SELECT COUNT(*) FROM in_effect WHERE rule_id='no-conditions-json';")
    [ "$result" -eq 1 ]

    # Should have no applicable entries
    result=$(sqlite3 data/rules.db "SELECT COUNT(*) FROM applicable WHERE rule_id='no-conditions-json';")
    [ "$result" -eq 0 ]
}

@test "json format preserves original content" {
    create_basic_json_rule "original.json" "preserve-json"

    "$INGEST_BIN" original.json

    run diff original.json data/rules/preserve-json/0.json
    [ "$status" -eq 0 ]
}

@test "json format generates UUID when no ID provided" {
    cat > no_id.json <<EOF
{
  "properties": {
    "name": "NoIDRule",
    "version": "1.0"
  },
  "in_effect": [
    {
      "in": "US-CA",
      "from": "2024-01-01T00:00",
      "to": "2024-12-31T23:59",
      "tz": "America/Los_Angeles"
    }
  ],
  "input_conditions": [
    {
      "expression": {
        "key": "age",
        "operator": ">=",
        "value": "18"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "00"}
      ]
    }
  ],
  "output_assertions": [
    {
      "key": "eligible",
      "value": "true",
      "scenarios": [
        {"case": "01"},
        {"case": "00"}
      ]
    }
  ]
}
EOF

    run "$INGEST_BIN" no_id.json
    [ "$status" -eq 0 ]

    # Should have created a directory with UUID name
    rule_count=$(ls -d data/rules/*/ 2>/dev/null | wc -l)
    [ "$rule_count" -eq 1 ]
}

@test "json format handles complex case patterns" {
    cat > complex_cases.json <<EOF
{
  "properties": {
    "id": "complex-cases-json"
  },
  "in_effect": [
    {
      "in": "US-CA",
      "from": "2024-01-01T00:00",
      "to": "2024-12-31T23:59",
      "tz": "America/Los_Angeles"
    }
  ],
  "input_conditions": [
    {
      "expression": {
        "key": "age",
        "operator": ">=",
        "value": "18"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "01"},
        {"case": "00"},
        {"case": "00"}
      ]
    },
    {
      "expression": {
        "key": "income",
        "operator": ">=",
        "value": "50000"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "00"},
        {"case": "01"},
        {"case": "00"}
      ]
    },
    {
      "expression": {
        "key": "credit",
        "operator": ">=",
        "value": "700"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "00"},
        {"case": "00"},
        {"case": "01"}
      ]
    }
  ],
  "output_assertions": [
    {
      "key": "eligible",
      "value": "true",
      "scenarios": [
        {"case": "01"},
        {"case": "00"},
        {"case": "00"},
        {"case": "00"}
      ]
    },
    {
      "key": "priority",
      "value": "high",
      "scenarios": [
        {"case": "00"},
        {"case": "01"},
        {"case": "01"},
        {"case": "00"}
      ]
    },
    {
      "key": "review",
      "value": "needed",
      "scenarios": [
        {"case": "00"},
        {"case": "00"},
        {"case": "01"},
        {"case": "01"}
      ]
    }
  ]
}
EOF

    run "$INGEST_BIN" complex_cases.json
    [ "$status" -eq 0 ]

    # Should extract 3 condition keys
    result=$(sqlite3 data/rules.db "SELECT COUNT(*) FROM applicable WHERE rule_id='complex-cases-json';")
    [ "$result" -eq 3 ]
}

@test "json format handles malformed JSON gracefully" {
    cat > malformed.json <<EOF
{
  "properties": {
    "id": "malformed"
  },
  "in_effect": [
    {
      "in": "US-CA",
      "from": "2024-01-01T00:00",
      "to": "2024-12-31T23:59",
      "tz": "America/Los_Angeles"
    }
  ],
  "input_conditions": [
    {
      "expression": {
        "key": "age",
        "operator": ">=",
        "value": "18"
      },
      "scenarios": [
        {"case": "01"},
        {"case": "00"}
      ]
    }
  ],
  "output_assertions": [
    {
      "key": "eligible",
      "value": "true",
      "scenarios": [
        {"case": "01"},
        {"case": "00"}
      ]
    }
  ]
  # Missing closing brace - malformed JSON
EOF

    run "$INGEST_BIN" malformed.json
    [ "$status" -ne 0 ]
}

@test "json format handles empty arrays" {
    cat > empty_arrays.json <<EOF
{
  "properties": {
    "id": "empty-arrays-json"
  },
  "in_effect": [
    {
      "in": "US-CA",
      "from": "2024-01-01T00:00",
      "to": "2024-12-31T23:59",
      "tz": "America/Los_Angeles"
    }
  ],
  "input_conditions": [],
  "output_assertions": []
}
EOF

    run "$INGEST_BIN" empty_arrays.json
    [ "$status" -eq 0 ]

    # Should have in_effect entry
    result=$(sqlite3 data/rules.db "SELECT COUNT(*) FROM in_effect WHERE rule_id='empty-arrays-json';")
    [ "$result" -eq 1 ]

    # Should have no applicable entries
    result=$(sqlite3 data/rules.db "SELECT COUNT(*) FROM applicable WHERE rule_id='empty-arrays-json';")
    [ "$result" -eq 0 ]
}