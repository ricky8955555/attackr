-- Your SQL goes here

DROP TABLE "solved";

CREATE TABLE "scores" (
	"id"	INTEGER,
	"user"	INTEGER NOT NULL,
	"challenge"	INTEGER NOT NULL,
	"time"	TIMESTAMP NOT NULL,
	"points"	REAL NOT NULL,
	PRIMARY KEY("id"),
	FOREIGN KEY("user") REFERENCES "users"("id") ON DELETE CASCADE,
	FOREIGN KEY("challenge") REFERENCES "challenges"("id") ON DELETE CASCADE
);

CREATE TABLE "solved" (
	"id"	INTEGER,
	"submission"	INTEGER NOT NULL,
	"score"	INTEGER,
	PRIMARY KEY("id"),
	FOREIGN KEY("submission") REFERENCES "submissions"("id") ON DELETE CASCADE,
	FOREIGN KEY("score") REFERENCES "scores"("id") ON DELETE CASCADE
);
