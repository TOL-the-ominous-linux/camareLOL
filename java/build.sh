#!/usr/bin/env bash
# Compile the Java catalog component and run its tests.
set -euo pipefail

cd "$(dirname "$0")"
OUT="${1:-out}"
mkdir -p "$OUT/classes" "$OUT/test-classes"

echo "==> compiling"
find src/main/java -name '*.java' -print0 | xargs -0 javac -d "$OUT/classes" -Xlint:all
find src/test/java -name '*.java' -print0 | xargs -0 javac -cp "$OUT/classes" -d "$OUT/test-classes" -Xlint:all

echo "==> running tests"
java -cp "$OUT/classes:$OUT/test-classes" com.camarelol.catalog.CatalogTest

echo "==> jar"
jar --create --file "$OUT/camarelol-catalog.jar" -C "$OUT/classes" .
