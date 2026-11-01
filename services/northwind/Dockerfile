FROM node:24-alpine
WORKDIR /app
COPY package*.json ./
RUN npm ci --omit=dev
COPY index.mjs read-model.mjs ./
USER node
EXPOSE 9080
CMD ["node", "index.mjs"]
