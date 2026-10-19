# Manifest entrypoints use AGP generated rules. kotlinx.serialization and Media3 carry
# their own consumer rules. Compose generated resources use literal resource paths.
# No app-wide keep/dontwarn or fallback signing. Device acceptance remains mandatory.
-keepattributes SourceFile,LineNumberTable
