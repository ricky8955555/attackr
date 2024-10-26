-- Your SQL goes here

CREATE TABLE "users" (
	"id"	INTEGER,
	"username"	TEXT NOT NULL,
	"password"	TEXT NOT NULL,
	"email"	TEXT NOT NULL,
	"contact"	TEXT NOT NULL,
	"enabled"	BOOLEAN NOT NULL,
	"role"	TEXT NOT NULL,
	PRIMARY KEY("id")
);

CREATE TABLE "problemsets" (
	"id"	INTEGER,
	"name"	TEXT NOT NULL,
	PRIMARY KEY("id")
);

CREATE TABLE "challenges" (
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
	PRIMARY KEY("id"),
	FOREIGN KEY("problemset") REFERENCES "problemsets"("id") ON DELETE SET NULL
);

CREATE TABLE "artifacts" (
	"id"	INTEGER NOT NULL,
	"user"	INTEGER,
	"challenge"	INTEGER NOT NULL,
	"flag"	TEXT NOT NULL,
	"info"	TEXT NOT NULL,
	"path"	TEXT NOT NULL,
	FOREIGN KEY("user") REFERENCES "users"("id") ON DELETE CASCADE,
	FOREIGN KEY("challenge") REFERENCES "challenges"("id") ON DELETE CASCADE,
	PRIMARY KEY("id")
);

CREATE TABLE "solved" (
	"id"	INTEGER,
	"submission"	INTEGER NOT NULL,
	"factor"	REAL NOT NULL,
	PRIMARY KEY("id"),
	FOREIGN KEY("submission") REFERENCES "submissions"("id") ON DELETE CASCADE
);

CREATE TABLE "submissions" (
	"id"	INTEGER,
	"user"	INTEGER NOT NULL,
	"challenge"	INTEGER NOT NULL,
	"time"	TIMESTAMP NOT NULL,
	"flag"	TEXT NOT NULL,
	PRIMARY KEY("id"),
	FOREIGN KEY("user") REFERENCES "users"("id") ON DELETE CASCADE,
	FOREIGN KEY("challenge") REFERENCES "challenges"("id") ON DELETE CASCADE
);
