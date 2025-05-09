-- CreateEnum
CREATE TYPE "ObjectStatus" AS ENUM ('created', 'mutated', 'deleted');

-- CreateTable
CREATE TABLE "objects" (
    "id" VARCHAR(66) NOT NULL,
    "owner" VARCHAR(66) NOT NULL,
    "type" VARCHAR(1000) NOT NULL,
    "status" "ObjectStatus" NOT NULL,
    "field_raw" TEXT NOT NULL,
    "fields" JSONB,
    "version" BIGINT NOT NULL,
    "updated_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT NOW(),

    CONSTRAINT "objects_pkey" PRIMARY KEY ("id")
);

-- CreateIndex
CREATE INDEX "objects_owner_idx" ON "objects"("owner");

-- CreateIndex
CREATE INDEX "objects_type_idx" ON "objects"("type");

-- CreateIndex
CREATE INDEX "objects_owner_type_idx" ON "objects"("owner", "type");
