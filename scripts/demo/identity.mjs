export const PERSON_NAME = "William Thøgersen";
export const PERSON_EMAIL = "william@example.com";

export const GIT_IDENTITY = [
  "-c", `user.name=${PERSON_NAME}`,
  "-c", `user.email=${PERSON_EMAIL}`,
  "-c", "commit.gpgsign=false",
  "-c", "core.hooksPath=/dev/null",
];
