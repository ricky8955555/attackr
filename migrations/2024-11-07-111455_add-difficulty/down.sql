-- This file should undo anything in `up.sql`

CREATE TABLE "new_challenges" (
	"id"	INTEGER,
	"name"	TEXT NOT NULL,
	"description"	TEXT NOT NULL,
	"path"	TEXT NOT NULL,
	"initial"	REAL NOT NULL,
	"points"	REAL NOT NULL,
	"problemset"	INTEGER,
	"attachments"	TEXT NOT NULL,
	"flag"	TEXT NOT NULL,
	"dynamic"	BOOLEAN NOT NULL,
	"public"	BOOLEAN NOT NULL,
    "difficulty"	INTEGER,
	PRIMARY KEY("id"),
	FOREIGN KEY("problemset") REFERENCES "problemsets"("id") ON DELETE SET NULL
);

INSERT INTO "new_challenges" SELECT * FROM "challenges";
DROP TABLE "challenges";

ALTER TABLE "new_challenges" DROP COLUMN "difficulty";
ALTER TABLE "new_challenges" RENAME TO "challenges";
DROP TABLE "difficulties";
