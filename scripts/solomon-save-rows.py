#!/usr/bin/env python3
"""Stream the Website's stored cloud saves into solomon-save-check.

Opens the Website's SQLite database read-only and writes one JSON line per WebGameSaves
row: {"slot": n, "revision": n, "document": "<stored text>"}. User IDs and row IDs are
not exported. The lines contain player documents, so the helper refuses to write them to
a terminal; pipe them into the checker:

    solomon-save-rows.py DATABASE | solomon-save-check --assets CLIENT_DIRECTORY

--find-sha256 HASH instead prints the Id, Slot and Revision of each row whose document
has that SHA-256, the identity the checker's summary gives a failing row.
"""

import argparse
import contextlib
import hashlib
import json
from pathlib import Path
import sqlite3
import sys


def stored_documents(database, columns):
    """Yield the selected columns and the document text of every row, in Id order."""
    # mode=ro never writes, and a missing file is an error rather than a new database.
    uri = database.resolve().as_uri() + '?mode=ro'
    with contextlib.closing(sqlite3.connect(uri, uri=True)) as db:
        query = f'SELECT {columns}, CAST(Document AS BLOB) FROM WebGameSaves ORDER BY Id'
        for *fields, document in db.execute(query):
            # Document is UTF-8 TEXT. Undecodable bytes become U+FFFD instead of stopping
            # the export; the checker then reports the row with the rest.
            yield fields, document.decode('utf-8', 'replace')


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('database', type=Path, help='Website SQLite database, opened read-only')
    parser.add_argument('--find-sha256', metavar='HASH',
                        help='print the rows whose document has this SHA-256 instead')
    args = parser.parse_args()
    if not args.find_sha256 and sys.stdout.isatty():
        parser.error('the rows contain player documents; pipe them into solomon-save-check')
    try:
        if args.find_sha256:
            wanted = args.find_sha256.lower()
            for (row_id, slot, revision), text in stored_documents(args.database,
                                                                   'Id, Slot, Revision'):
                if hashlib.sha256(text.encode()).hexdigest() == wanted:
                    print(json.dumps({'id': row_id, 'slot': slot, 'revision': revision}))
            return
        for (slot, revision), text in stored_documents(args.database, 'Slot, Revision'):
            # ASCII escapes keep the stream independent of the terminal or locale encoding.
            sys.stdout.write(json.dumps({'slot': slot, 'revision': revision,
                                         'document': text}) + '\n')
    except sqlite3.Error as error:
        # SQLite errors name the file, table or lock, never row contents.
        parser.exit(1, f'solomon-save-rows: {error}\n')


if __name__ == '__main__':
    main()
