-- CreateEnum
CREATE TYPE "ObjectStatus" AS ENUM ('created', 'mutated', 'deleted');

-- CreateTable
CREATE TABLE "objects" (
    "id" VARCHAR(66) NOT NULL,
    "owner" VARCHAR(66) NOT NULL,
    "type" VARCHAR(1000),
    "status" "ObjectStatus" NOT NULL,
    "field_raw" TEXT NOT NULL,
    "fields" JSONB,
    "display" JSONB NOT NULL DEFAULT '{}',
    "last_tx_digist" VARCHAR(66),
    "version" BIGINT NOT NULL,
    "updated_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),

    CONSTRAINT "objects_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "object_types" (
    "id" VARCHAR(1000) NOT NULL,
    "fields" TEXT NOT NULL,
    "version" BIGINT NOT NULL,
    "updated_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),

    CONSTRAINT "object_types_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "packages" (
    "id" VARCHAR(66) NOT NULL,
    "virtual_id" VARCHAR(66) NOT NULL,
    "serialized" TEXT NOT NULL,
    "version" BIGINT NOT NULL,
    "last_tx_digist" VARCHAR(66),
    "updated_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),

    CONSTRAINT "packages_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "displays" (
    "id" SERIAL NOT NULL,
    "object_type" TEXT NOT NULL,
    "fields" JSONB NOT NULL,
    "bcs" TEXT NOT NULL,
    "version" INTEGER NOT NULL,
    "last_tx_digist" VARCHAR(66),
    "updated_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT NOW()
);

-- CreateIndex
CREATE INDEX "objects_owner_idx" ON "objects"("owner");

-- CreateIndex
CREATE INDEX "objects_type_idx" ON "objects"("type");

-- CreateIndex
CREATE INDEX "objects_owner_type_idx" ON "objects"("owner", "type");

-- CreateIndex
CREATE INDEX "objects_updated_at_idx" ON "objects"("updated_at" DESC);

-- CreateIndex
CREATE INDEX "objects_id_version_idx" ON "objects"("id", "version" DESC);

-- CreateIndex
CREATE INDEX "packages_id_version_idx" ON "packages"("id", "version" DESC);

-- CreateIndex
CREATE INDEX "packages_virtual_id_version_idx" ON "packages"("virtual_id", "version" DESC);

-- CreateIndex
CREATE UNIQUE INDEX "displays_object_type_key" ON "displays"("object_type");

-- CreateIndex
CREATE INDEX "displays_object_type_version_idx" ON "displays"("object_type", "version" DESC);
