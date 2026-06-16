#!/usr/bin/env python3
"""Batch generate tcase_uuid in RFC4122 UUID v5 format.

Usage:
    python generate_tcase_uuid.py <repo> <branch> '["case_name1", "case_name2"]'

Output:
    JSON object mapping case_name -> tcase_uuid
"""

import sys
import json
import uuid


def generate(repo: str, branch: str, case_names: list) -> dict:
    return {
        name: str(uuid.uuid5(uuid.NAMESPACE_URL, f"tcase://{repo}/{branch}/{name}"))
        for name in case_names
    }


if __name__ == "__main__":
    if len(sys.argv) != 4:
        print('Usage: python generate_tcase_uuid.py <repo> <branch> \'["name1", "name2"]\'')
        sys.exit(1)
    repo = sys.argv[1]
    branch = sys.argv[2]
    case_names = json.loads(sys.argv[3])
    print(json.dumps(generate(repo, branch, case_names), ensure_ascii=False))
