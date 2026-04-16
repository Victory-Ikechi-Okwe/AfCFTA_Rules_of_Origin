"""
AfCFTA Wholly Obtained Product Checker
---------------------------------------
Supports two rules:
  1. Mineral Rule      (wholly_obtained_rule)
  2. Plant Rule        (wholly_obtained_agricultural_rule)

The user selects the product type first and the app routes
to the correct rule and conditions automatically.
"""

import socket
import json
import os
from datetime import datetime, timezone

# ─────────────────────────────────────────────────────────────────────────────
# CONFIGURATION
# ─────────────────────────────────────────────────────────────────────────────

_SOCKET_PATH = "/tmp/rs.sock"
RECORDS_FILE = os.path.join(os.path.dirname(__file__), "product_records.json")


# ─────────────────────────────────────────────────────────────────────────────
# AfCFTA STATE PARTIES
# ─────────────────────────────────────────────────────────────────────────────

AFCFTA_STATE_PARTIES = [
    "Algeria", "Angola", "Benin", "Botswana", "Burkina Faso",
    "Burundi", "Cabo Verde", "Cameroon", "Central African Republic",
    "Chad", "Comoros", "Congo (Brazzaville)", "Congo (DRC)", "Djibouti",
    "Egypt", "Equatorial Guinea", "Eritrea", "Eswatini", "Ethiopia",
    "Gabon", "Gambia", "Ghana", "Guinea", "Guinea-Bissau", "Ivory Coast",
    "Kenya", "Lesotho", "Liberia", "Libya", "Madagascar", "Malawi",
    "Mali", "Mauritania", "Mauritius", "Morocco", "Mozambique", "Namibia",
    "Niger", "Nigeria", "Rwanda", "Sao Tome and Principe", "Senegal",
    "Seychelles", "Sierra Leone", "Somalia", "South Africa", "South Sudan",
    "Sudan", "Tanzania", "Togo", "Tunisia", "Uganda", "Zambia", "Zimbabwe",
]


# ─────────────────────────────────────────────────────────────────────────────
# RULE DEFINITIONS
# ─────────────────────────────────────────────────────────────────────────────

_RULES = {
    "mineral": {
        "label":   "Mineral Product",
        "rule_id": "wholly_obtained_rule",
        "conditions": [
            ("classified_as_mineral", "What type of product is this?", [
                ("Natural occurring Mineral", "true"),
                ("Non-natural occuring Mineral", "false"),
            ]),
            ("extraction_location", "How was this product obtained?", [
                ("Extracted from the ground",        "extracted_from_ground"),
                ("Extracted from the sea bed",       "extracted_from_seabed"),
                ("Extracted from below the sea bed", "extracted_from_below_seabed"),
            ]),
            ("unclos_compliant", "Does the production method comply with international laws?", [
                ("Yes", "true"),
                ("No",  "false"),
            ]),
        ],
    },
    "plant": {
        "label":   "Plant Product",
        "rule_id": "wholly_obtained_agricultural_rule",
        "conditions": [
            ("classification_location", "How is this product classified?", [
                ("Aquatic plant", "aquatic"),
                ("Plant product", "plant"),
                ("Vegetable",     "vegetable"),
                ("Fruit",         "fruit"),
            ]),
            ("grown_in_afcfta", "Was this product grown in an AfCFTA State Party?", [
                ("Yes", "true"),
                ("No",  "false"),
            ]),
            ("harvested_in_afcfta", "Was this product harvested in an AfCFTA State Party?", [
                ("Yes", "true"),
                ("No",  "false"),
            ]),
        ],
    },
}


# ─────────────────────────────────────────────────────────────────────────────
# LEGAL PHRASES
# ─────────────────────────────────────────────────────────────────────────────

_OUTCOME_PHRASES = {
    "TRUE":                      "is confirmed as being",
    "FALSE":                     "is determined to not be",
    "MAYBE":                     "may potentially be",
    "TRUE (and possibly false)": "is provisionally considered",
    "INVALID":                   "cannot be determined as",
}

# Maps internal outcomes to user-friendly display labels
_OUTCOME_LABELS = {
    "TRUE":                      "YES",
    "FALSE":                     "NO",
    "MAYBE":                     "YES OR NO",
    "TRUE (and possibly false)": "YES AND NO",
    "INVALID":                   "UNDETERMINED",
}


# ─────────────────────────────────────────────────────────────────────────────
# COUNTRY SELECTION — paged display
# ─────────────────────────────────────────────────────────────────────────────

def pick_country(prompt: str) -> str:
    """Display AfCFTA State Parties in pages of 18 and return the selected one."""
    print(f"\n  {prompt}\n")

    page_size = 18
    total     = len(AFCFTA_STATE_PARTIES)
    page      = 0

    while True:
        # Print current page in 3 columns
        start = page * page_size
        end   = min(start + page_size, total)
        col_width = 30
        for i in range(start, end):
            entry = f"  {i + 1:>2}. {AFCFTA_STATE_PARTIES[i]}"
            print(f"{entry:<{col_width + 6}}", end="\n" if (i - start + 1) % 3 == 0 else "")
        print()

        # Navigation options
        options = []
        if end < total:
            options.append("N = next page")
        if page > 0:
            options.append("P = previous page")
        options.append(f"1-{total} = select country")
        print(f"  [{' | '.join(options)}]")

        answer = input("\n  Enter choice: ").strip().lower()

        if answer == "n" and end < total:
            page += 1
        elif answer == "p" and page > 0:
            page -= 1
        elif answer.isdigit() and 1 <= int(answer) <= total:
            return AFCFTA_STATE_PARTIES[int(answer) - 1]
        else:
            print("  Invalid choice. Please try again.")


# ─────────────────────────────────────────────────────────────────────────────
# PRODUCT TYPE SELECTION
# ─────────────────────────────────────────────────────────────────────────────

def pick_rule() -> dict:
    """Ask the user what type of product they are checking and return the matching rule."""
    print("\n  What type of product are you checking?\n")
    rule_keys = list(_RULES.keys())
    for i, key in enumerate(rule_keys, 1):
        print(f"    {i}. {_RULES[key]['label']}")

    while True:
        answer = input(f"\n  Enter choice (1-{len(rule_keys)}): ").strip()
        if answer.isdigit() and 1 <= int(answer) <= len(rule_keys):
            return _RULES[rule_keys[int(answer) - 1]]
        print("  Invalid choice. Please try again.")


# ─────────────────────────────────────────────────────────────────────────────
# PRODUCT INFORMATION
# ─────────────────────────────────────────────────────────────────────────────

def ask_product_info() -> tuple[str, str]:
    """Collect product name and company name from the user."""
    print("\n" + "=" * 60)
    print("   AfCFTA Wholly Obtained Product Checker")
    print("=" * 60 + "\n")

    while True:
        product = input("  Product name: ").strip()
        if product:
            break
        print("  Product name cannot be empty.\n")

    while True:
        company = input("  Company name: ").strip()
        if company:
            break
        print("  Company name cannot be empty.\n")

    return product, company


# ─────────────────────────────────────────────────────────────────────────────
# CONDITION COLLECTION
# ─────────────────────────────────────────────────────────────────────────────

def ask_conditions(rule: dict) -> tuple[dict, str, str]:
    """Collect conditions for the selected rule. Returns document, producing and destination country."""
    document = {}

    # Originating state
    producing_country = pick_country(
        "Select the country where this product was produced:"
    )
    document["produced_in_afcfta_state"] = "true"
    print(f"\n  Country of production: {producing_country}")

    # Destination state
    destination_country = pick_country(
        "Select the country this product is being exported to:"
    )
    document["exported_to_afcfta_state"] = "true"
    print(f"\n  Country of destination: {destination_country}\n")

    # Rule-specific conditions
    for key, question, options in rule["conditions"]:
        print(f"  {question}")
        for i, (label, _) in enumerate(options, 1):
            print(f"    {i}. {label}")

        while True:
            answer = input(f"  Enter choice (1-{len(options)}): ").strip()
            if answer.isdigit() and 1 <= int(answer) <= len(options):
                _, value = options[int(answer) - 1]

                # Mineral rule — extraction location sets three keys from one answer
                if key == "extraction_location":
                    document["extracted_from_ground"]       = "true" if value == "extracted_from_ground"       else "false"
                    document["extracted_from_seabed"]       = "true" if value == "extracted_from_seabed"       else "false"
                    document["extracted_from_below_seabed"] = "true" if value == "extracted_from_below_seabed" else "false"

                # Plant rule — classification sets four keys from one answer
                elif key == "classification_location":
                    document["classified_as_aquatic"]   = "true" if value == "aquatic"   else "false"
                    document["classified_as_plant"]     = "true" if value == "plant"     else "false"
                    document["classified_as_vegetable"] = "true" if value == "vegetable" else "false"
                    document["classified_as_fruit"]     = "true" if value == "fruit"     else "false"

                else:
                    document[key] = value

                break
            print("  Invalid choice. Please try again.\n")
        print()

    return document, producing_country, destination_country


# ─────────────────────────────────────────────────────────────────────────────
# EVALUATION ENGINE
# ─────────────────────────────────────────────────────────────────────────────

def _evaluate(rule_id: str, document: dict) -> dict | None:
    """Send document to the internal evaluation engine and return the result."""
    package = json.dumps({
        "rule_id": rule_id,
        "document": document
    }).encode("utf-8")

    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
        try:
            s.connect(_SOCKET_PATH)
        except (FileNotFoundError, ConnectionRefusedError):
            print("\n  The compliance checking service is currently unavailable.")
            print("  Please contact your system administrator.\n")
            return None

        s.sendall(package)
        s.shutdown(socket.SHUT_WR)

        response = b""
        while True:
            chunk = s.recv(4096)
            if not chunk:
                break
            response += chunk

    return json.loads(response.decode("utf-8"))


# ─────────────────────────────────────────────────────────────────────────────
# LEGAL STATEMENT BUILDER
# ─────────────────────────────────────────────────────────────────────────────

def _build_statement(key: str, outcome: str, producing: str, destination: str) -> str:
    """Build a formal legal statement for a given assertion outcome."""
 
    phrase = _OUTCOME_PHRASES.get(outcome, "has an undetermined status regarding being")

    templates = {
        "wholly_obtained": (
            f"It is hereby determined that this product {phrase} wholly obtained "
            f"within {producing}, an AfCFTA State Party, "
            f"under the AfCFTA Rules of Origin."
        ),
        "qualifies_originating": (
            f"This product {phrase} an originating product of {producing}, "
            f"an AfCFTA State Party, under the AfCFTA Rules of Origin."
        ),
        "deemed_originating": (
            f"This product {phrase} deemed to be an originating product of {producing}, "
            f"an AfCFTA State Party, under the AfCFTA Rules of Origin."
        ),
        "eligible_preferential": (
            f"This product {phrase} eligible for preferential tariff treatment "
            f"upon entry into {destination}, an AfCFTA State Party, "
            f"under the AfCFTA Agreement."
        ),
        "deemed_preferential": (
            f"This product {phrase} deemed eligible for preferential tariff treatment "
            f"upon entry into {destination}, an AfCFTA State Party, "
            f"under the AfCFTA Agreement."
        ),
    }

    return templates.get(
        key,
        f"This product {phrase} {key.replace('_', ' ')} under the applicable rules."
    )


# ─────────────────────────────────────────────────────────────────────────────
# RECORD SAVING
# ─────────────────────────────────────────────────────────────────────────────

def _save_record(product, company, producing, destination, document, result):
    """Save the full evaluation record to the JSON records file."""
    record = {
        "timestamp":         datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "product_name":      product,
        "company_name":      company,
        "originating_state": producing,
        "destination_state": destination,
        "conditions":        document,
        "_rule_id":          result.get("rule_id"),
        "_rule_version":     result.get("version"),
        "_scenario":         result.get("scenario"),
        "findings": [
            {
                "outcome":   _OUTCOME_LABELS.get(a["outcome"], a["outcome"]),
                "statement": _build_statement(
                    a["key"], a["outcome"], producing, destination
                ),
            }
            for a in result.get("assertions", [])
        ],
    }

    if os.path.exists(RECORDS_FILE):
        try:
            with open(RECORDS_FILE, "r") as f:
                records = json.load(f)
        except json.JSONDecodeError:
            records = []
    else:
        records = []

    records.append(record)
    with open(RECORDS_FILE, "w") as f:
        json.dump(records, f, indent=2)

    print("  Evaluation record saved.")


# ─────────────────────────────────────────────────────────────────────────────
# DISPLAY RESULTS
# ─────────────────────────────────────────────────────────────────────────────

def display_results(result, product, company, producing, destination):
    """Display formal legal compliance findings to the user."""
    if "error" in result:
        print("\n  The compliance check could not be completed.")
        print("  Please contact your system administrator.\n")
        return

    print("\n" + "=" * 60)
    print("  AfCFTA COMPLIANCE DETERMINATION")
    print("=" * 60)
    print(f"  Date:                   {datetime.now(timezone.utc).strftime('%d %B %Y %H:%M UTC')}")
    print(f"  Company:                {company}")
    print(f"  Product:                {product}")
    print(f"  Country of Origin:      {producing}")
    print(f"  Country of Destination: {destination}")
    print("=" * 60)
    print("\n  FORMAL FINDINGS:\n")

    if result.get("scenario") == "No matching scenario":
        print("  Based on the information provided, this product does not")
        print("  qualify as wholly obtained under the AfCFTA Rules of Origin.\n")
        print("=" * 60 + "\n")
        return

    for i, assertion in enumerate(result["assertions"], 1):
        statement = _build_statement(
            assertion["key"], assertion["outcome"], producing, destination
        )
        outcome = assertion["outcome"]
        label   = _OUTCOME_LABELS.get(outcome, outcome)

        if outcome == "TRUE":
            colour = "\033[92m"
        elif outcome == "FALSE":
            colour = "\033[91m"
        elif outcome == "MAYBE":
            colour = "\033[93m"
        else:
            colour = "\033[94m"
        reset = "\033[0m"

        print(f"  {i}. [{colour}{label}{reset}] {statement}\n")

    print("=" * 60 + "\n")


# ─────────────────────────────────────────────────────────────────────────────
# MAIN
# ─────────────────────────────────────────────────────────────────────────────

def main():
    """Run the AfCFTA Wholly Obtained Product Checker."""
    product, company = ask_product_info()

    # Route to the correct rule based on product type
    rule = pick_rule()

    document, producing, destination = ask_conditions(rule)

    print("  Checking compliance, please wait...")
    result = _evaluate(rule["rule_id"], document)
    print (result)
    if result:
        display_results(result, product, company, producing, destination)
        _save_record(product, company, producing, destination, document, result)


if __name__ == "__main__":
    main()
