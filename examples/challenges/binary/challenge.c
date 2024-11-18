#include <stdio.h>
#include <string.h>

const char flag[] = "flag{}";

int main() {
    char buf[50];
    puts("please type your flag:");
    fgets(buf, sizeof(buf), stdin);
    buf[strcspn(buf, "\r\n")] = 0;  // remove newline char
    if (strcmp(flag, buf) == 0) {
        puts("cheers! you've got the flag!");
    } else {
        puts("oh no, that's not correct!");
    }
}
