#!/bin/bash
set -e

IMAGE_NAME="monxas/putioarr:latest"
DOCKERFILE_DIR="$(dirname "$0")"

# Check for buildx
if ! docker buildx version > /dev/null 2>&1; then
  echo "docker buildx is not installed. Please install Docker Buildx."
  exit 1
fi

# Ensure buildx builder exists
if ! docker buildx inspect multiarch-builder > /dev/null 2>&1; then
  docker buildx create --name multiarch-builder --use
else
  docker buildx use multiarch-builder
fi

echo "Building and pushing $IMAGE_NAME for linux/amd64 and linux/arm64..."
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t $IMAGE_NAME \
  -f "$DOCKERFILE_DIR/Dockerfile" \
  --push \
  "$DOCKERFILE_DIR/.."

echo "Build and push complete!"
