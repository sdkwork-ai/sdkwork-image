#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FAMILY_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
IMAGE_ROOT="$(cd "${FAMILY_ROOT}/../.." && pwd)"
WORKSPACE_ROOT="$(cd "${FAMILY_ROOT}/../../.." && pwd)"
GENERATOR_PATH="${WORKSPACE_ROOT}/sdkwork-sdk-generator/bin/sdkgen.js"
INPUT_PATH="${FAMILY_ROOT}/openapi/sdkwork-image-backend-api.sdkgen.yaml"
SDK_NAME="sdkwork-image-backend-sdk"
BASE_URL="${BASE_URL:-http://localhost:8080}"
SDK_VERSION="${SDK_VERSION:-1.0.0}"
API_PREFIX="/backend/v3/api"
LANGUAGES="${LANGUAGES:-typescript,dart,python,go,java,kotlin,swift,csharp,flutter,rust,php,ruby}"

if [[ ! -f "${GENERATOR_PATH}" ]]; then
  echo "Canonical SDK generator not found: ${GENERATOR_PATH}" >&2
  exit 1
fi

if [[ ! -f "${INPUT_PATH}" ]]; then
  node "${IMAGE_ROOT}/sdks/materialize-image-v3-openapi-boundaries.mjs"
fi

package_name() {
  case "$1" in
    typescript) echo "@sdkwork/image-backend-sdk" ;;
    dart|flutter) echo "sdkwork_image_backend_sdk" ;;
    python|swift|rust|ruby) echo "sdkwork-image-backend-sdk" ;;
    go) echo "github.com/sdkwork/sdkwork-image-backend-sdk" ;;
    java|kotlin) echo "com.sdkwork:sdkwork-image-backend-sdk" ;;
    csharp) echo "SDKWork.Image.BackendSdk" ;;
    php) echo "sdkwork/image-backend-sdk" ;;
    *) echo "sdkwork-image-backend-sdk-$1" ;;
  esac
}

namespace_args() {
  case "$1" in
    java|kotlin) printf '%s\n' "--namespace" "com.sdkwork.image.backend.sdk" ;;
    csharp) printf '%s\n' "--namespace" "SDKWork.Image.BackendSdk" ;;
    php) printf '%s\n' "--namespace" "SDKWork\\Image\\BackendSdk" ;;
  esac
}

IFS=',' read -r -a language_array <<< "${LANGUAGES}"
for language in "${language_array[@]}"; do
  language="$(echo "${language}" | xargs)"
  [[ -z "${language}" ]] && continue
  language_workspace="${FAMILY_ROOT}/${SDK_NAME}-${language}"
  output_path="${language_workspace}/generated/server-openapi"
  # bash 3.2 has no mapfile (macOS /usr/bin/bash): read the lines into the
  # same indexed array so SDK regeneration also works off Linux.
  ns_args=()
  while IFS= read -r ns_line; do ns_args+=("$ns_line"); done < <(namespace_args "${language}")
  rm -rf "${output_path}"
  node "${GENERATOR_PATH}" generate \
    -i "${INPUT_PATH}" \
    -o "${output_path}" \
    -n "${SDK_NAME}" \
    -t backend \
    -l "${language}" \
    --fixed-sdk-version "${SDK_VERSION}" \
    --base-url "${BASE_URL}" \
    --api-prefix "${API_PREFIX}" \
    --package-name "$(package_name "${language}")" \
    --standard-profile sdkwork-v3 \
    --sdk-root "${FAMILY_ROOT}" \
    --sdk-name "${SDK_NAME}" \
    --no-sync-published-version \
    "${ns_args[@]}"
done
