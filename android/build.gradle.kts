// Only declare a plugin here if it must be loaded once rather than per-subproject
// https://discuss.gradle.org/t/why-duplicate-plugins-in-top-level-build-scripts/49087/2
// https://www.reddit.com/r/androiddev/comments/1errttm/comment/li1vm93/
plugins {
    alias(libs.plugins.android.application) apply false
}
