#!/usr/bin/env bash
# Build the Moltis gateway image on Pegasus and push to GHCR.
set -euo pipefail

IMAGE_REPO="${IMAGE_REPO:-ghcr.io/sp-937-215/moltis}"
COMMIT_SHA="${CI_COMMIT_SHA:-${CI_COMMIT:-${WOODPECKER_COMMIT_SHA:-${WOODPECKER_COMMIT:-}}}}"
BRANCH_RAW="${CI_COMMIT_BRANCH:-${WOODPECKER_BRANCH:-${CI_COMMIT_REF:-}}}"
BRANCH_SAFE="$(echo "${BRANCH_RAW}" | tr '/:' '--' | tr -c 'A-Za-z0-9._-' '-' | sed 's/--*/-/g; s/^-//; s/-$//')"
SHA12="$(echo "${COMMIT_SHA}" | cut -c1-12)"

if [[ -z "${SHA12}" ]]; then
  echo "ERROR: empty commit SHA (CI_COMMIT_SHA / WOODPECKER_COMMIT not set)"
  env | grep -E 'CI_|WOODPECKER_' | sort || true
  exit 1
fi

TAG_SHA="${SHA12}"
TAG_BRANCH="${BRANCH_SAFE:-branch}"
TAG_FORK="fork"

echo "Building ${IMAGE_REPO} from commit ${SHA12} (branch=${BRANCH_RAW:-unknown})"
echo "Docker:"
docker version

# Prefer linux/amd64 for typical TrueNAS SCALE hosts. Override with PLATFORM=.
PLATFORM="${PLATFORM:-linux/amd64}"

# Bake the upstream date version so the in-app update banner (releases.json
# YYYYMMDD.NN compare) stays quiet. A `fork-<sha>` string is always treated
# as older than any stable release. Override with MOLTIS_VERSION=.
MOLTIS_VERSION="${MOLTIS_VERSION:-20260902.03}"

docker build \
  --platform "${PLATFORM}" \
  -f Dockerfile \
  --build-arg "MOLTIS_VERSION=${MOLTIS_VERSION}" \
  --label "org.opencontainers.image.revision=${COMMIT_SHA}" \
  --label "org.opencontainers.image.version=${MOLTIS_VERSION}" \
  -t "${IMAGE_REPO}:${TAG_SHA}" \
  -t "${IMAGE_REPO}:${TAG_BRANCH}" \
  -t "${IMAGE_REPO}:${TAG_FORK}" \
  .

if [[ -n "${GHCR_TOKEN:-}" && -n "${GHCR_USERNAME:-}" ]]; then
  echo "${GHCR_TOKEN}" | docker login ghcr.io -u "${GHCR_USERNAME}" --password-stdin
elif [[ -f /root/.docker/config.json ]] || [[ -f "${HOME}/.docker/config.json" ]]; then
  echo "Using existing Docker registry credentials"
else
  echo "ERROR: no GHCR credentials."
  echo "On Pegasus run once: docker login ghcr.io -u SP-937-215"
  echo "Or set Woodpecker secrets GHCR_USERNAME + GHCR_TOKEN (write:packages)."
  exit 1
fi

echo "Pushing to GHCR..."
docker push "${IMAGE_REPO}:${TAG_SHA}"
docker push "${IMAGE_REPO}:${TAG_BRANCH}"
docker push "${IMAGE_REPO}:${TAG_FORK}"

echo "Published:"
echo "  ${IMAGE_REPO}:${TAG_FORK}"
echo "  ${IMAGE_REPO}:${TAG_BRANCH}"
echo "  ${IMAGE_REPO}:${TAG_SHA}"
echo "TrueNAS Custom App → Image repository: ${IMAGE_REPO}  Tag: ${TAG_FORK}"
