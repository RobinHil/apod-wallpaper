# The site is static, so the image that ships is nginx plus a directory. Node
# only ever exists in the build stage and never reaches the published layer.

FROM node:24-alpine AS build
WORKDIR /app

# pnpm comes from corepack, and the version is the one pinned by
# `packageManager` in package.json rather than whatever is newest.
RUN corepack enable

COPY package.json pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile

COPY . .

# Where this image will be served from. The defaults are for a container that
# owns the root of its host; the repository's own default is the subdirectory
# GitHub Pages serves the project from. Both end up in the canonical URL, the
# hreflang pair, the sitemap and the social card URLs.
ARG SITE_BASE=/
ARG SITE_ORIGIN=http://localhost:8080
ENV SITE_BASE=$SITE_BASE
ENV SITE_ORIGIN=$SITE_ORIGIN

# A NASA key of your own, if you have one. Left empty the page falls back to
# NASA's DEMO_KEY, and to its own painted sky when that key is exhausted.
ARG VITE_NASA_API_KEY=""
ENV VITE_NASA_API_KEY=$VITE_NASA_API_KEY

# Builds the client bundle, the server bundle, and then the two prerendered
# pages: / in English and /fr/ in French.
RUN pnpm build


FROM nginx:1.27-alpine AS serve

COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/dist /usr/share/nginx/html

EXPOSE 80

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
  CMD wget -q -O /dev/null http://127.0.0.1/ || exit 1

CMD ["nginx", "-g", "daemon off;"]
