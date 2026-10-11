#!/usr/bin/env python3
"""Select the newest stable Core release, never a provider-only tag."""

import json
import re
import sys


def latest_core_tag(releases: list[dict]) -> str:
    eligible = []
    for release in releases:
        tag = release["tagName"]
        if release.get("isDraft") or release.get("isPrerelease"):
            continue
        if match := re.fullmatch(r"v?([0-9]+)\.([0-9]+)\.([0-9]+)", tag):
            eligible.append((tuple(map(int, match.groups())), tag))
    if not eligible:
        raise ValueError("no stable Core release with a numeric version tag; publish Core before testing a provider")
    return max(eligible)[1]


if __name__ == "__main__":
    try:
        print(latest_core_tag(json.load(sys.stdin)))
    except (ValueError, KeyError, TypeError) as error:
        sys.exit(str(error))
