#!/bin/sh

VERSION="$1"
if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    exit 1
fi

# AWK State Machine:
# 1. Look for the target version header.
# 2. Once found, print lines until the NEXT header is encountered.
# 3. Exit immediately when the next header starts to avoid unnecessary processing.

awk -v ver="$VERSION" '
    /^## \[/ {
        if ($0 ~ "## \\[" ver "\\]") {
            found = 1
            print $0
            next
        } else if (found) {
            exit # Stop as soon as we hit the next version
        }
    }
    found { print $0 }
' CHANGELOG.md