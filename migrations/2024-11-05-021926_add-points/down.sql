-- This file should undo anything in `up.sql`

DROP TABLE "scores";
DROP TABLE "solved";

CREATE TABLE "solved" (
	"id"	INTEGER,
	"submission"	INTEGER NOT NULL,
	"factor"	REAL NOT NULL,
	PRIMARY KEY("id"),
	FOREIGN KEY("submission") REFERENCES "submissions"("id") ON DELETE CASCADE
);
