// SPDX-License-Identifier: AGPL-3.0-only
#include <onnxruntime_c_api.h>
#include <cstring>
#include <iostream>
int main() {
  const auto* base = OrtGetApiBase();
  if (!base || std::strcmp(base->GetVersionString(), "1.28.3")) return 1;
  const auto* api = base->GetApi(ORT_API_VERSION);
  if (!api) return 2;
  OrtEnv* env = nullptr;
  OrtStatus* status = api->CreateEnv(ORT_LOGGING_LEVEL_WARNING, "siny-smoke", &env);
  if (status) { std::cerr << api->GetErrorMessage(status); api->ReleaseStatus(status); return 3; }
  status = api->DisableTelemetryEvents(env);
  if (status) { std::cerr << api->GetErrorMessage(status); api->ReleaseStatus(status); api->ReleaseEnv(env); return 4; }
  api->ReleaseEnv(env);
  std::cout << "ORT C API and environment smoke passed: " << base->GetVersionString() << '\n';
  return 0;
}
