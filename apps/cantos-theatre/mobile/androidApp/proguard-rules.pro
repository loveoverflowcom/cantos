# Narrow keep rules for the optimized demo/QA build (#245 part C). The base file is the AGP
# `proguard-android-optimize.txt`; the AndroidX, Compose, Kotlin and kotlinx.serialization
# libraries ship their own consumer rules. Nothing here is a blanket `-keep`, and warnings are
# never silenced globally: add a rule only for a class that is reached by name at runtime and
# prove it with the R8 output (`mapping.txt`, `usage.txt`, `missing_rules.txt`) of this variant.
