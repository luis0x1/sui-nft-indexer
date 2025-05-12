/*
  Warnings:

  - A unique constraint covering the columns `[virtual_id]` on the table `packages` will be added. If there are existing duplicate values, this will fail.
  - Added the required column `virtual_id` to the `packages` table without a default value. This is not possible if the table is not empty.

*/
-- AlterTable
ALTER TABLE "displays" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "object_types" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "objects" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "packages" ADD COLUMN     "virtual_id" VARCHAR(66) NOT NULL,
ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- CreateIndex
CREATE INDEX "packages_virtual_id_version_idx" ON "packages"("virtual_id", "version" DESC);

-- CreateIndex
CREATE UNIQUE INDEX "packages_virtual_id_key" ON "packages"("virtual_id");
